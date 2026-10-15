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
    #[serde(default)]
    pub boards: Vec<PathBuf>,
}
impl Project {
    pub fn root_of(&self, path: &Path) -> Option<std::path::PathBuf> {
        let mut current = path.to_owned();
        for _ in 0..1000 {
            let p = self.pages.iter().find(|p| p.file == current)?;
            if let Some(parent) = &p.parent {
                current = parent.clone();
            } else {
                return Some(current);
            }
        }
        None
    }
    pub fn page_context(&self, path: &Path) -> Option<(usize, usize)> {
        let root = self.root_of(path)?;
        let pages = self
            .pages
            .iter()
            .filter(|p| self.root_of(&p.file).as_ref() == Some(&root))
            .collect::<Vec<_>>();
        Some((pages.iter().position(|p| p.file == path)? + 1, pages.len()))
    }
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
        boards: fs::read_dir(&folder)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "kicad_pcb"))
            .collect(),
    };
    let mut seen = HashSet::new();
    let mut identities = HashSet::new();
    fn walk(
        path: &Path,
        parent: Option<PathBuf>,
        sheet_uuid: Option<String>,
        name: String,
        seen: &mut HashSet<PathBuf>,
        pages: &mut Vec<Page>,
        identities: &mut HashSet<String>,
    ) -> Result<()> {
        let canonical = jenga_core::document::normalized_path(&path.canonicalize()?);
        ensure!(pages.len() < 1000, "Project exceeds 1000 pages");
        if !seen.insert(canonical.clone()) {
            return Ok(());
        }
        let d = Document::open(&canonical)?;
        if !identities.insert(d.root_uuid().to_owned()) {
            return Ok(());
        }
        let title = jenga_core::page::TitleBlock::read(&d).title;
        let name = if !title.is_empty() { title } else { name };
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
                    identities,
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
    if root.is_file() && root.extension().is_some_and(|e| e == "kicad_sch") {
        walk(
            &root,
            None,
            None,
            project.name.clone(),
            &mut seen,
            &mut project.pages,
            &mut identities,
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
        walk(
            &p,
            None,
            None,
            name,
            &mut seen,
            &mut project.pages,
            &mut identities,
        )?;
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
        .unwrap_or(false);
    let next_page = discover(&parent_path)?
        .page_context(&parent_path)
        .map(|(_, n)| n + 1)
        .unwrap_or(2);
    let mut child = Document::new();
    if let Some(paper) = parent.root.child("paper") {
        child.root.set_child(paper.clone());
    }
    let mut title = jenga_core::page::TitleBlock::read(parent);
    title.title = name.into();
    title.write(&mut child);
    let id = uuid::Uuid::new_v4().to_string();
    if hierarchical {
        let at = params
            .get("at")
            .map(|_| crate::engine::point(params, "at"))
            .transpose()?
            .unwrap_or_else(|| {
                let (_, b) = jenga_core::scene::Scene::from_document(parent).content_bounds();
                (b.0 + 12.7, 25.4)
            });
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
                    Node::string(format!(
                        "{}/{}",
                        parent
                            .root
                            .child("sheet_instances")
                            .and_then(|n| n.children("path").find(|n| n.value(1) != "/"))
                            .map(|n| n.value(1).to_owned())
                            .unwrap_or_else(|| format!("/{}", parent.root_uuid())),
                        id
                    )),
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
        doc.page_context = project.page_context(&page.file);
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

pub fn delete_page(doc: &Document, p: &Value) -> Result<Value> {
    ensure!(
        !doc.dirty,
        "Save the active schematic before deleting a page"
    );
    let project = discover(doc.path.as_ref().context("Open a saved project")?)?;
    let target = std::path::PathBuf::from(crate::engine::string(p, "path")?).canonicalize()?;
    let target = jenga_core::document::normalized_path(&target);
    let page = project
        .pages
        .iter()
        .find(|n| n.file == target)
        .context("Page is not in this project")?;
    ensure!(
        project
            .project_file
            .as_ref()
            .is_none_or(|p| p.with_extension("kicad_sch") != target),
        "Use delete_project to delete the root schematic/project"
    );
    let folder = crate::symbol_editor::personal_folder()?
        .join("recovery-pages")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&folder)?;
    let destination = folder.join(target.file_name().unwrap());
    let mut changed: Option<(std::path::PathBuf, Vec<u8>)> = None;
    if let Some(parent) = &page.parent {
        let bytes = fs::read(parent)?;
        let mut d = Document::open(parent)?;
        let id = page
            .sheet_uuid
            .as_deref()
            .context("Missing native sheet UUID")?;
        d.root
            .items_mut()
            .retain(|n| n.name() != "sheet" || n.child("uuid").is_none_or(|u| u.value(1) != id));
        d.dirty = true;
        d.save(parent)?;
        changed = Some((parent.clone(), bytes));
    }
    let manifest = json!({"original":target,"saved":destination,"parent":changed.as_ref().map(|v|&v.0),"parent_before":changed.as_ref().map(|v|String::from_utf8_lossy(&v.1).to_string())});
    if let Err(e) = jenga_core::document::atomic_write(
        &folder.join("restore.json"),
        &serde_json::to_vec_pretty(&manifest)?,
        false,
    ) {
        if let Some((path, bytes)) = &changed {
            jenga_core::document::atomic_write(path, bytes, false)?;
        }
        return Err(e);
    }
    if let Err(e) = fs::rename(&target, &destination) {
        if let Some((path, bytes)) = &changed {
            jenga_core::document::atomic_write(path, bytes, false)?;
        }
        return Err(e.into());
    }
    let _ = crate::catalog::record("delete_page", Some(&target), None);
    Ok(
        json!({"deleted":target,"recovery":folder,"open_after":page.parent.clone().or_else(||project.project_file.as_ref().map(|p|p.with_extension("kicad_sch"))).or_else(||project.pages.iter().find(|p|p.file!=target).map(|p|p.file.clone()))}),
    )
}

pub fn rename_page(doc: &Document, p: &Value) -> Result<Value> {
    ensure!(!doc.dirty, "Save current edits before renaming a page");
    let project = discover(doc.path.as_ref().context("Save project first")?)?;
    let name = crate::engine::string(p, "name")?;
    ensure!(
        !name.trim().is_empty() && name.len() < 1000,
        "Use a nonempty page name"
    );
    let path = p
        .get("path")
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
        .unwrap_or(doc.path.clone().unwrap());
    let path = jenga_core::document::normalized_path(&path.canonicalize()?);
    let page = project
        .pages
        .iter()
        .find(|n| n.file == path)
        .context("Page is not in project")?;
    let mut child = Document::open(&path)?;
    let before = child.clone();
    let mut title = jenga_core::page::TitleBlock::read(&child);
    title.title = name.into();
    title.write(&mut child);
    child.save(&path)?;
    if let Some(parent) = &page.parent {
        let result = (|| -> Result<()> {
            let mut d = Document::open(parent)?;
            for n in d
                .root
                .items_mut()
                .iter_mut()
                .filter(|n| n.name() == "sheet")
            {
                if n.child("uuid")
                    .is_some_and(|u| Some(u.value(1)) == page.sheet_uuid.as_deref())
                {
                    for property in n
                        .items_mut()
                        .iter_mut()
                        .filter(|n| n.name() == "property" && n.value(1) == "Sheetname")
                    {
                        property.items_mut()[2] = Node::string(name);
                    }
                }
            }
            d.dirty = true;
            d.save(parent)?;
            Ok(())
        })();
        if let Err(e) = result {
            jenga_core::document::atomic_write(
                &path,
                before.root.document_text().as_bytes(),
                false,
            )?;
            return Err(e);
        }
    }
    Ok(
        json!({"path":path,"name":name,"note":"Native title/sheet name changed; filenames and UUIDs are preserved"}),
    )
}
pub fn reorder_pages(doc: &Document, p: &Value) -> Result<Value> {
    ensure!(!doc.dirty, "Save before reordering pages");
    let project = discover(doc.path.as_ref().context("Save project first")?)?;
    let paths = p["pages"]
        .as_array()
        .context("pages required")?
        .iter()
        .map(|v| std::path::PathBuf::from(v.as_str().unwrap_or("")))
        .collect::<Vec<_>>();
    ensure!(
        paths.len() == project.pages.len()
            && paths.iter().collect::<HashSet<_>>().len() == paths.len()
            && project.pages.iter().all(|n| paths.contains(&n.file)),
        "Supply each project page path exactly once"
    );
    let rank = |path: &std::path::Path| paths.iter().position(|p| p == path).unwrap_or(usize::MAX);
    for page in &project.pages {
        if let Some(parent) = &page.parent {
            ensure!(
                rank(parent) < rank(&page.file),
                "Parent pages must precede their children"
            );
        }
    }
    let roots = project
        .pages
        .iter()
        .filter(|p| p.parent.is_none())
        .map(|p| &p.file)
        .collect::<Vec<_>>();
    let requested_roots = paths
        .iter()
        .filter(|p| roots.contains(p))
        .collect::<Vec<_>>();
    ensure!(
        roots == requested_roots,
        "Independent schematics keep their root order; reorder sibling pages instead"
    );
    let mut writes = vec![];
    for page in &project.pages {
        let mut d = Document::open(&page.file)?;
        let old = fs::read(&page.file)?;
        let mut sheets = d.root.children("sheet").cloned().collect::<Vec<_>>();
        sheets.sort_by_key(|s| rank(&page.file.parent().unwrap().join(property(s, "Sheetfile"))));
        let mut next = sheets.into_iter();
        for n in d.root.items_mut().iter_mut() {
            if n.name() == "sheet" {
                *n = next.next().unwrap();
            }
        }
        if let Some(instances) = d.root.child_mut("sheet_instances") {
            for path in instances
                .items_mut()
                .iter_mut()
                .filter(|n| n.name() == "path")
            {
                path.set_child(Node::list(
                    "page",
                    vec![Node::string(
                        (paths
                            .iter()
                            .filter(|p| project.root_of(p) == project.root_of(&page.file))
                            .position(|p| p == &page.file)
                            .unwrap()
                            + 1)
                        .to_string(),
                    )],
                ));
            }
        }
        writes.push((page.file.clone(), old, d.root.document_text().into_bytes()));
    }
    for (i, (path, _, new)) in writes.iter().enumerate() {
        if let Err(e) = jenga_core::document::atomic_write(path, new, true) {
            for (path, old, _) in &writes[..i] {
                let _ = jenga_core::document::atomic_write(path, old, false);
            }
            return Err(e);
        }
    }
    Ok(
        json!({"pages":paths,"note":"Native sibling sheet order and page numbers updated; independent schematic filenames retain their project order"}),
    )
}

pub fn move_sibling(project: &Project, path: &Path, up: bool) -> Result<Vec<std::path::PathBuf>> {
    let i = project
        .pages
        .iter()
        .position(|p| p.file == path)
        .context("Page missing")?;
    let parent = project.pages[i]
        .parent
        .as_ref()
        .context("Root pages cannot be moved")?;
    let j = if up {
        (0..i)
            .rev()
            .find(|&j| project.pages[j].parent.as_ref() == Some(parent))
    } else {
        (i + 1..project.pages.len()).find(|&j| project.pages[j].parent.as_ref() == Some(parent))
    }
    .context("No sibling in that direction")?;
    let block = |root: &Path| {
        let mut set = std::collections::HashSet::from([root.to_owned()]);
        loop {
            let n = set.len();
            for p in &project.pages {
                if p.parent.as_ref().is_some_and(|p| set.contains(p)) {
                    set.insert(p.file.clone());
                }
            }
            if n == set.len() {
                break;
            }
        }
        project
            .pages
            .iter()
            .filter(|p| set.contains(&p.file))
            .map(|p| p.file.clone())
            .collect::<Vec<_>>()
    };
    let a = block(path);
    let b = block(&project.pages[j].file);
    let mut out = vec![];
    for (k, p) in project.pages.iter().enumerate() {
        if k == i.min(j) {
            if up {
                out.extend(a.clone());
                out.extend(b.clone());
            } else {
                out.extend(b.clone());
                out.extend(a.clone());
            }
        }
        if !a.contains(&p.file) && !b.contains(&p.file) {
            out.push(p.file.clone());
        }
    }
    Ok(out)
}
