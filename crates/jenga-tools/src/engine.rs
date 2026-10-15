use crate::{journal, render};
use anyhow::{Context, Result, bail, ensure};
use jenga_core::{
    checks,
    document::{Document, create_project, editable, position, property},
    export,
    page::TitleBlock,
    scene::{Point, Scene, points},
    sexpr::Node,
};
use jenga_libraries::Libraries;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing string: {key}"))
}
pub fn point(v: &Value, key: &str) -> Result<Point> {
    let a = v
        .get(key)
        .and_then(Value::as_array)
        .with_context(|| format!("Missing [x,y] millimetres: {key}"))?;
    ensure!(a.len() == 2, "{key} must contain two coordinates");
    let x = a[0].as_f64().context("Invalid x")?;
    let y = a[1].as_f64().context("Invalid y")?;
    ensure!(
        x.is_finite() && y.is_finite() && x.abs() < 100000. && y.abs() < 100000.,
        "Coordinates out of range"
    );
    Ok((x, y))
}
pub fn target(doc: &Document, v: &Value) -> Result<usize> {
    if let Some(id) = v.get("uuid").and_then(Value::as_str) {
        return doc
            .root
            .items()
            .iter()
            .position(|n| editable(n.name()) && n.child("uuid").is_some_and(|n| n.value(1) == id))
            .context("Object UUID not found");
    }
    let reference = string(v, "reference")?;
    let unit = v.get("unit").and_then(Value::as_u64);
    let matches: Vec<_> = doc
        .root
        .items()
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            n.name() == "symbol"
                && property(n, "Reference") == reference
                && unit
                    .is_none_or(|u| n.child("unit").map(|n| n.number(1) as u64).unwrap_or(1) == u)
        })
        .map(|(i, _)| i)
        .collect();
    ensure!(
        matches.len() == 1,
        "Reference must identify one symbol; specify unit or UUID"
    );
    Ok(matches[0])
}
pub fn inspect(doc: &Document) -> Value {
    let scene = Scene::from_document(doc);
    let objects:Vec<_>=doc.root.items().iter().enumerate().filter(|(_,n)|editable(n.name())).map(|(i,n)|{
        let pins:Vec<_>=scene.pins.iter().filter(|p|p.object==i).map(|p|json!({"number":p.number,"electrical_type":p.kind,"name":p.name,"hidden":p.hidden,"outward":p.outward,"at_mm":p.at})).collect();
        json!({"uuid":n.child("uuid").map(|n|n.value(1)),"kind":n.name(),"reference":property(n,"Reference"),"value":property(n,"Value"),"library_id":n.child("lib_id").map(|n|n.value(1)),"unit":n.child("unit").map(|n|n.number(1)),"footprint":property(n,"Footprint"),"at_mm":position(n),"bounds_mm":scene.object_bounds(i),"pins":pins,"points_mm":points(n,"pts"),"text":if matches!(n.name(),"text"|"label"|"global_label"|"hierarchical_label"){n.value(1)}else{""}})
    }).collect();
    json!({"file":doc.path,"project":doc.project_name(),"dirty":doc.dirty,"revision":journal::revision(doc),"root_uuid":doc.root_uuid(),"paper_mm":export::paper_size(doc),"paper":doc.root.child("paper").map(|n|n.value(1)),"title_block":TitleBlock::read(doc),"objects":objects,"missing_symbols":scene.missing,"warnings":doc.warnings()})
}
pub fn findings(doc: &Document) -> Value {
    json!(checks::findings(doc))
}
/// A whole edit request is validated on a clone before any document mutation is committed.
pub fn edit(doc: &mut Document, libs: &mut Libraries, request: &Value) -> Result<Value> {
    if let Some(expected) = request.get("expected_revision").and_then(Value::as_str) {
        ensure!(
            expected == journal::revision(doc),
            "Revision conflict: inspect the document again before editing"
        );
    }
    let actions = request
        .get("actions")
        .and_then(Value::as_array)
        .context("edit requires actions array")?;
    ensure!(actions.len() <= 10000, "Too many actions");
    let mut candidate = doc.clone();
    let mut results = vec![];
    for action in actions {
        results.push(action_apply(&mut candidate, libs, action)?);
    }
    *doc = candidate;
    Ok(json!({"revision":journal::revision(doc),"results":results,"dirty":doc.dirty}))
}
fn action_apply(doc: &mut Document, libs: &mut Libraries, a: &Value) -> Result<Value> {
    let op = string(a, "op")?;
    match op {
        "add_symbol" => {
            let id = string(a, "library_id")?;
            let at = point(a, "at")?;
            let def = libs.symbol(id)?;
            let unit = a.get("unit").and_then(Value::as_u64).unwrap_or(1);
            let max = def
                .children("symbol")
                .filter_map(|s| s.value(1).rsplit('_').nth(1)?.parse::<u64>().ok())
                .max()
                .unwrap_or(1)
                .max(1);
            ensure!(
                (1..=max).contains(&unit),
                "Unit is not defined by this symbol"
            );
            let i = doc.add_symbol(id, &def, at.0, at.1, unit as u32);
            let rotation = a.get("rotation").and_then(Value::as_i64).unwrap_or(0);
            ensure!(rotation % 90 == 0, "Rotation must be a multiple of 90");
            for _ in 0..rotation.rem_euclid(360) / 90 {
                doc.rotate(i);
            }
            if let Some(properties) = a.get("properties").and_then(Value::as_object) {
                for (key, value) in properties {
                    doc.set_property(
                        i,
                        key,
                        value.as_str().context("Property values must be strings")?,
                    );
                }
            }
            return Ok(
                json!({"uuid":doc.root.items()[i].child("uuid").map(|n|n.value(1)),"reference":property(&doc.root.items()[i],"Reference")}),
            );
        }
        "wire" => {
            let pts = a
                .get("points")
                .and_then(Value::as_array)
                .context("wire needs points")?;
            ensure!(
                pts.len() >= 2 && pts.len() <= 10000,
                "Invalid wire point count"
            );
            let mut coords = Vec::new();
            for p in pts {
                coords.push(point(&json!({"at":p}), "at")?);
            }
            for w in coords.windows(2) {
                doc.add_wire(w[0], w[1]);
            }
        }
        "connect_pins" => {
            let scene = Scene::from_document(doc);
            let from = pin(doc, &scene, a.get("from").context("Missing from pin")?)?;
            let to = pin(doc, &scene, a.get("to").context("Missing to pin")?)?;
            let horizontal = a
                .get("horizontal_first")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let direction = |at: Point| {
                scene
                    .pins
                    .iter()
                    .find(|p| p.at == at)
                    .map(|p| p.outward)
                    .unwrap_or((0., 0.))
            };
            let fd = direction(from);
            let td = direction(to);
            let a = (from.0 + fd.0 * 2.54, from.1 + fd.1 * 2.54);
            let b = (to.0 + td.0 * 2.54, to.1 + td.1 * 2.54);
            let (c, d) = if horizontal {
                let x = (a.0 + b.0) / 2.;
                ((x, a.1), (x, b.1))
            } else {
                let y = (a.1 + b.1) / 2.;
                ((a.0, y), (b.0, y))
            };
            for segment in [from, a, c, d, b, to].windows(2) {
                doc.add_wire(segment[0], segment[1]);
            }
        }
        "junction" | "no_connect" => {
            let at = if let Some(p) = a.get("pin") {
                pin(doc, &Scene::from_document(doc), p)?
            } else {
                point(a, "at")?
            };
            doc.add_point(op, at.0, at.1);
        }
        "text" | "label" | "global_label" | "hierarchical_label" => {
            let at = point(a, "at")?;
            doc.add_text(op, string(a, "text")?, at.0, at.1);
        }
        "move" => {
            let i = target(doc, a)?;
            let at = point(a, "at")?;
            ensure!(
                doc.root.items()[i].child("at").is_some(),
                "Use translate for wires"
            );
            let (x, y, _) = position(&doc.root.items()[i]);
            doc.translate(i, at.0 - x, at.1 - y);
        }
        "translate" => {
            let i = target(doc, a)?;
            let d = point(a, "delta")?;
            doc.translate(i, d.0, d.1);
        }
        "rotate" => {
            let i = target(doc, a)?;
            doc.rotate(i);
        }
        "delete" => {
            let i = target(doc, a)?;
            doc.remove(i);
        }
        "refresh_symbol" | "replace_symbol" => {
            let i = target(doc, a)?;
            ensure!(
                doc.root.items()[i].name() == "symbol",
                "Target must be symbol"
            );
            let id = if op == "replace_symbol" {
                string(a, "library_id")?.to_owned()
            } else {
                doc.root.items()[i]
                    .child("lib_id")
                    .context("Symbol has no library ID")?
                    .value(1)
                    .to_owned()
            };
            let mut def = libs.symbol(&id)?;
            let unit = if op == "replace_symbol" {
                a.get("unit").and_then(Value::as_u64).unwrap_or(1)
            } else {
                doc.root.items()[i]
                    .child("unit")
                    .map(|n| n.number(1) as u64)
                    .unwrap_or(1)
            };
            let max = def
                .children("symbol")
                .filter_map(|s| s.value(1).rsplit('_').nth(1)?.parse::<u64>().ok())
                .max()
                .unwrap_or(1)
                .max(1);
            ensure!(
                (1..=max).contains(&unit),
                "Unit is not defined by this symbol"
            );
            def.items_mut()[1] = Node::string(&id);
            let embedded = doc
                .root
                .child_mut("lib_symbols")
                .context("No embedded symbol table")?;
            if let Some(existing) = embedded
                .items_mut()
                .iter_mut()
                .find(|s| s.name() == "symbol" && s.value(1) == id)
            {
                *existing = def;
            } else {
                embedded.items_mut().push(def);
            }
            let n = &mut doc.root.items_mut()[i];
            n.set_child(Node::list("lib_id", vec![Node::string(&id)]));
            n.items_mut().retain(|n| n.name() != "lib_name");
            if op == "replace_symbol" {
                n.set_child(Node::list(
                    "unit",
                    vec![Node::atom(
                        a.get("unit").and_then(Value::as_u64).unwrap_or(1),
                    )],
                ));
                n.items_mut().retain(|n| n.name() != "pin");
            }
            doc.dirty = true;
        }
        "set_property" => {
            let i = target(doc, a)?;
            ensure!(
                doc.root.items()[i].name() == "symbol",
                "Properties apply to symbols"
            );
            doc.set_property(i, string(a, "key")?, string(a, "value")?);
        }
        "set_text" => {
            let i = target(doc, a)?;
            ensure!(
                matches!(
                    doc.root.items()[i].name(),
                    "text" | "label" | "global_label" | "hierarchical_label"
                ),
                "Object has no editable text"
            );
            doc.root.items_mut()[i].items_mut()[1] = Node::string(string(a, "text")?);
            doc.dirty = true;
        }
        "title_block" => {
            let mut block = TitleBlock::read(doc);
            if let Some(fields) = a.get("fields").and_then(Value::as_object) {
                for (k, v) in fields {
                    match k.as_str() {
                        "title" => {
                            block.title = v.as_str().context("title must be a string")?.into()
                        }
                        "date" => block.date = v.as_str().context("date must be a string")?.into(),
                        "revision" => {
                            block.revision = v.as_str().context("revision must be a string")?.into()
                        }
                        "company" => {
                            block.company = v.as_str().context("company must be a string")?.into()
                        }
                        "comments" => {
                            let arr = v.as_array().context("comments must be an array")?;
                            ensure!(arr.len() <= 9, "At most 9 comments");
                            block.comments = arr
                                .iter()
                                .map(|v| {
                                    v.as_str()
                                        .map(str::to_owned)
                                        .context("Comment must be string")
                                })
                                .collect::<Result<_>>()?;
                        }
                        _ => bail!("Unknown title field {k}"),
                    }
                }
            } else {
                bail!("title_block needs fields");
            }
            block.write(doc);
        }
        "paper" => {
            let name = string(a, "name")?;
            ensure!(
                [
                    "A0", "A1", "A2", "A3", "A4", "A5", "USLetter", "USLegal", "User"
                ]
                .contains(&name),
                "Unsupported paper"
            );
            let mut values = vec![Node::string(name)];
            if name == "User" {
                let p = point(a, "size_mm")?;
                ensure!(p.0 >= 40. && p.1 >= 40., "Page too small");
                values.push(Node::atom(p.0));
                values.push(Node::atom(p.1));
            }
            if a.get("portrait").and_then(Value::as_bool) == Some(true) {
                values.push(Node::atom("portrait"));
            }
            doc.root.set_child(Node::list("paper", values));
            doc.dirty = true;
        }
        _ => bail!("Unknown edit operation {op}"),
    }
    Ok(json!({"op":op,"ok":true}))
}
fn pin(doc: &Document, scene: &Scene, v: &Value) -> Result<Point> {
    let i = target(doc, v)?;
    let number = string(v, "number")?;
    scene
        .pins
        .iter()
        .find(|p| p.object == i && p.number == number)
        .map(|p| p.at)
        .context("Pin not found in selected symbol unit")
}

pub struct Engine {
    pub doc: Option<Document>,
    pub libraries: Option<Libraries>,
    pub workspace: PathBuf,
    pub undo: Vec<Document>,
    pub redo: Vec<Document>,
}
impl Engine {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            doc: None,
            libraries: None,
            workspace,
            undo: vec![],
            redo: vec![],
        }
    }
    pub fn resolve(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.workspace.join(p)
        }
    }
    pub fn document(&self) -> Result<&Document> {
        self.doc.as_ref().context("No document open")
    }
    pub fn load_libraries(&mut self) -> Result<()> {
        if self.libraries.is_none() {
            let project = self
                .doc
                .as_ref()
                .and_then(|d| d.path.as_ref())
                .and_then(|p| p.parent())
                .unwrap_or(&self.workspace);
            let mut libs = Libraries::discover(&[], Some(project));
            libs.index()?;
            self.libraries = Some(libs);
        }
        Ok(())
    }
    pub fn request(&mut self, method: &str, p: &Value) -> Result<Value> {
        match method {
            "open" => {
                ensure!(
                    !self.doc.as_ref().is_some_and(|d| d.dirty)
                        || p.get("discard_changes") == Some(&Value::Bool(true)),
                    "Unsaved changes: save first or explicitly discard_changes"
                );
                let mut path = self.resolve(string(p, "path")?);
                if path.extension().is_some_and(|e| e == "kicad_pro") {
                    path.set_extension("kicad_sch");
                }
                self.doc = Some(Document::open(&path)?);
                self.libraries = None;
                self.undo.clear();
                self.redo.clear();
                Ok(inspect(self.document()?))
            }
            "new" | "create_schematic" => {
                ensure!(
                    !self.doc.as_ref().is_some_and(|d| d.dirty),
                    "Save existing changes first"
                );
                let mut doc = Document::new();
                if let Some(path) = p.get("path").and_then(Value::as_str) {
                    let path = self.resolve(path);
                    ensure!(!path.exists(), "File already exists");
                    doc.path = Some(path);
                }
                doc.dirty = true;
                if method == "create_schematic" {
                    let path = doc.path.clone().context("create_schematic needs path")?;
                    doc.save(&path)?;
                }
                self.doc = Some(doc);
                self.undo.clear();
                self.redo.clear();
                self.libraries = None;
                Ok(inspect(self.document()?))
            }
            "create_project" => {
                ensure!(
                    !self.doc.as_ref().is_some_and(|d| d.dirty),
                    "Save existing changes first"
                );
                let folder = p
                    .get("folder")
                    .and_then(Value::as_str)
                    .map(|s| self.resolve(s))
                    .unwrap_or_else(|| self.workspace.clone());
                self.doc = Some(create_project(&folder, string(p, "name")?)?);
                self.undo.clear();
                self.redo.clear();
                self.libraries = None;
                Ok(inspect(self.document()?))
            }
            "inspect" | "ui_state" => Ok(if let Some(doc) = &self.doc {
                inspect(doc)
            } else {
                json!({"file":null,"running_app":false,"workspace":self.workspace})
            }),
            "search_symbols" => {
                self.load_libraries()?;
                let libs = self.libraries.as_ref().unwrap();
                let query = p.get("query").and_then(Value::as_str).unwrap_or("");
                let library = p.get("library").and_then(Value::as_str).unwrap_or("");
                let results = libs.search(query, library);
                let limit = p
                    .get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(100)
                    .min(1000) as usize;
                Ok(
                    json!({"total":results.len(),"results":results.iter().take(limit).map(|e|json!({"id":e.id,"description":e.description,"keywords":e.keywords,"footprint":e.footprint,"source":e.source})).collect::<Vec<_>>()}),
                )
            }
            "get_symbol" => {
                self.load_libraries()?;
                let id = string(p, "id")?;
                let def = self.libraries.as_mut().unwrap().symbol(id)?;
                let scene = Scene::symbol_preview_unit(
                    &def,
                    p.get("unit").and_then(Value::as_u64).unwrap_or(1) as u32,
                );
                Ok(
                    json!({"id":id,"definition":def.encoded(),"bounds_mm":scene.content_bounds(),"pins":scene.pins.iter().map(|p|json!({"number":p.number,"at_mm":p.at,"type":p.kind,"name":p.name,"outward":p.outward})).collect::<Vec<_>>()}),
                )
            }
            "get_footprint" => {
                self.load_libraries()?;
                let id = string(p, "id")?;
                let def = self.libraries.as_ref().unwrap().footprint(id)?;
                Ok(
                    json!({"id":id,"definition":def.encoded(),"bounds_mm":Scene::footprint_preview(&def).content_bounds()}),
                )
            }
            "edit" => {
                self.load_libraries()?;
                let before = self.document()?.clone();
                let result = edit(
                    self.doc.as_mut().context("No document open")?,
                    self.libraries.as_mut().unwrap(),
                    p,
                )?;
                let actor = p.get("actor").and_then(Value::as_str).unwrap_or("external");
                if let Err(e) = journal::record(&before, self.document()?, actor, "edit") {
                    self.doc = Some(before);
                    return Err(e);
                }
                self.undo.push(before);
                if self.undo.len() > 100 {
                    self.undo.remove(0);
                }
                self.redo.clear();
                Ok(result)
            }
            "undo" | "redo" => {
                let previous = if method == "undo" {
                    self.undo.pop()
                } else {
                    self.redo.pop()
                }
                .context("No history available in this session")?;
                let before = self.document()?.clone();
                if method == "undo" {
                    self.redo.push(before.clone());
                } else {
                    self.undo.push(before.clone());
                }
                self.doc = Some(previous);
                self.doc.as_mut().unwrap().dirty = true;
                journal::record(
                    &before,
                    self.document()?,
                    p.get("actor").and_then(Value::as_str).unwrap_or("external"),
                    method,
                )?;
                Ok(inspect(self.document()?))
            }
            "save" => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .map(|s| self.resolve(s))
                    .or_else(|| self.doc.as_ref()?.path.clone())
                    .context("Save requires path for new document")?;
                let before = self.document()?.clone();
                self.doc.as_mut().unwrap().save(&path)?;
                let history_warning = journal::record(
                    &before,
                    self.document()?,
                    p.get("actor").and_then(Value::as_str).unwrap_or("external"),
                    "save",
                )
                .err()
                .map(|e| e.to_string());
                Ok(
                    json!({"saved":path,"revision":journal::revision(self.document()?),"history_warning":history_warning}),
                )
            }
            "export" | "screenshot" => {
                let path = self.resolve(string(p, "path")?);
                let doc = self.document()?;
                let kind = p
                    .get("format")
                    .and_then(Value::as_str)
                    .unwrap_or(if method == "screenshot" { "png" } else { "svg" });
                match kind {
                    "svg" => export::svg(doc, &path)?,
                    "pdf" => export::pdf(doc, &path)?,
                    "csv" => export::bom(doc, &path)?,
                    "png" => render::png(
                        doc,
                        &path,
                        p.get("width").and_then(Value::as_u64).unwrap_or(1800) as u32,
                    )?,
                    _ => bail!("Unsupported export format"),
                };
                Ok(json!({"path":path,"format":kind}))
            }
            "erc" => Ok(findings(self.document()?)),
            "history" => Ok(json!(journal::read(
                self.document()?,
                p.get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(50)
                    .min(500) as usize
            )?)),
            "list_projects" => {
                let folder = p
                    .get("folder")
                    .and_then(Value::as_str)
                    .map(|s| self.resolve(s))
                    .unwrap_or_else(|| self.workspace.clone());
                Ok(json!(files(&folder, 0)?))
            }
            "read_file" => {
                let path = self.resolve(string(p, "path")?);
                ensure!(
                    std::fs::metadata(&path)?.len() <= 32 * 1024 * 1024,
                    "File too large"
                );
                Ok(json!({"path":path,"text":std::fs::read_to_string(path)?}))
            }
            "close" => {
                ensure!(
                    !self.doc.as_ref().is_some_and(|d| d.dirty)
                        || p.get("discard_changes") == Some(&Value::Bool(true)),
                    "Save first or explicitly discard_changes"
                );
                self.doc = None;
                self.undo.clear();
                self.redo.clear();
                Ok(json!({"closed":true}))
            }
            "rename" => {
                let before = self.document()?.clone();
                let mut result =
                    crate::projects::rename(self.doc.as_mut().context("No document open")?, p)?;
                let history_warning = journal::record(
                    &before,
                    self.document()?,
                    p.get("actor").and_then(Value::as_str).unwrap_or("external"),
                    "rename",
                )
                .err()
                .map(|e| e.to_string());
                result["history_warning"] = json!(history_warning);
                self.libraries = None;
                self.undo.clear();
                self.redo.clear();
                Ok(result)
            }
            "create_library" => {
                let name = string(p, "name")?;
                crate::projects::valid_name(name)?;
                let kind = p.get("kind").and_then(Value::as_str).unwrap_or("symbols");
                ensure!(
                    kind == "symbols" || kind == "footprints",
                    "kind must be symbols or footprints"
                );
                if kind == "symbols" {
                    let mut request = p.clone();
                    request["source"] =
                        json!("(kicad_symbol_lib (version 20250114) (generator jenga))");
                    return self.import(&request);
                }
                let project = self
                    .document()?
                    .path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .context("Save schematic before creating a library")?
                    .to_owned();
                let folder = project.join("libraries").join(format!("{name}.pretty"));
                ensure!(!folder.exists(), "Library folder exists");
                let tablepath = project.join("fp-lib-table");
                let mut table = if tablepath.exists() {
                    jenga_core::sexpr::parse(&std::fs::read_to_string(&tablepath)?)?
                } else {
                    Node::list(
                        "fp_lib_table",
                        vec![Node::list("version", vec![Node::atom(7)])],
                    )
                };
                ensure!(
                    table.name() == "fp_lib_table"
                        && !table
                            .children("lib")
                            .any(|n| n.child("name").is_some_and(|n| n.value(1) == name)),
                    "Existing or invalid library table entry"
                );
                table.items_mut().push(Node::list(
                    "lib",
                    vec![
                        Node::list("name", vec![Node::string(name)]),
                        Node::list("type", vec![Node::string("KiCad")]),
                        Node::list(
                            "uri",
                            vec![Node::string(format!(
                                "${{KIPRJMOD}}/libraries/{name}.pretty"
                            ))],
                        ),
                        Node::list("options", vec![Node::string("")]),
                        Node::list("descr", vec![Node::string("Authored project library")]),
                    ],
                ));
                std::fs::create_dir_all(&folder)?;
                jenga_core::document::atomic_write(
                    &tablepath,
                    table.document_text().as_bytes(),
                    true,
                )?;
                self.libraries = None;
                journal::record_resource(
                    self.document()?,
                    &folder.to_string_lossy(),
                    "",
                    "",
                    p.get("actor").and_then(Value::as_str).unwrap_or("external"),
                    "create_library",
                )?;
                Ok(json!({"path":folder,"registered":true}))
            }
            "list_libraries" => {
                self.load_libraries()?;
                let libs = self.libraries.as_ref().unwrap();
                Ok(
                    json!({"sources":libs.sources,"symbol_count":libs.entries.len(),"footprint_count":libs.footprint_ids().len(),"warnings":libs.warnings}),
                )
            }
            "search_footprints" => {
                self.load_libraries()?;
                let query = p
                    .get("query")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_lowercase();
                let matches: Vec<_> = self
                    .libraries
                    .as_ref()
                    .unwrap()
                    .footprint_ids()
                    .into_iter()
                    .filter(|id| {
                        query
                            .split_whitespace()
                            .all(|token| id.to_lowercase().contains(token))
                    })
                    .collect();
                let count = matches.len();
                Ok(
                    json!({"total":count,"results":matches.into_iter().take(p.get("limit").and_then(Value::as_u64).unwrap_or(100).min(1000)as usize).collect::<Vec<_>>()}),
                )
            }
            "preview_symbol" | "preview_footprint" => {
                self.load_libraries()?;
                let id = string(p, "id")?;
                let scene = if method == "preview_symbol" {
                    Scene::symbol_preview_unit(
                        &self.libraries.as_mut().unwrap().symbol(id)?,
                        p.get("unit").and_then(Value::as_u64).unwrap_or(1) as u32,
                    )
                } else {
                    Scene::footprint_preview(&self.libraries.as_ref().unwrap().footprint(id)?)
                };
                let path = self.resolve(string(p, "path")?);
                crate::render::preview(
                    &scene,
                    &path,
                    p.get("width").and_then(Value::as_u64).unwrap_or(800) as u32,
                )?;
                Ok(json!({"path":path,"id":id}))
            }
            "read_library" => {
                let path = self.resolve(string(p, "path")?);
                ensure!(
                    std::fs::metadata(&path)?.len() <= 16 * 1024 * 1024,
                    "Library too large"
                );
                let text = std::fs::read_to_string(&path)?;
                let root = jenga_core::sexpr::parse(&text)?;
                ensure!(
                    matches!(root.name(), "kicad_symbol_lib" | "footprint" | "module"),
                    "Not a native library file"
                );
                Ok(
                    json!({"path":path,"source":text,"revision":journal::revision_text(&root.encoded())}),
                )
            }
            "import_library" | "write_library" => self.import(p),
            _ => bail!("Unknown method {method}"),
        }
    }
    fn import(&mut self, p: &Value) -> Result<Value> {
        let name = string(p, "name")?;
        ensure!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "Invalid library name"
        );
        let project = self
            .document()?
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .context("Save schematic before importing a project library")?
            .to_owned();
        let source = if let Some(source) = p.get("source").and_then(Value::as_str) {
            ensure!(source.len() <= 16 * 1024 * 1024, "Library too large");
            source.to_owned()
        } else if let Some(url) = p.get("url").and_then(Value::as_str) {
            ensure!(
                url.starts_with("https://") && !url.contains(['\r', '\n']),
                "Only HTTPS download URLs supported"
            );
            let output = std::process::Command::new("curl.exe")
                .args([
                    "--fail",
                    "--silent",
                    "--show-error",
                    "--location",
                    "--proto",
                    "=https",
                    "--proto-redir",
                    "=https",
                    "--max-time",
                    "30",
                    "--max-filesize",
                    "16777216",
                    url,
                ])
                .output()?;
            ensure!(
                output.status.success(),
                "Download failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout)?
        } else {
            let path = self.resolve(string(p, "path")?);
            ensure!(
                std::fs::metadata(&path)?.len() <= 16 * 1024 * 1024,
                "Library too large"
            );
            std::fs::read_to_string(path)?
        };
        let root = jenga_core::sexpr::parse(&source)?;
        let symbols = root.name() == "kicad_symbol_lib";
        ensure!(
            symbols || matches!(root.name(), "footprint" | "module"),
            "Expected native .kicad_sym or .kicad_mod data"
        );
        if symbols && p.get("source").is_none() {
            ensure!(
                root.children("symbol").next().is_some(),
                "Library contains no symbols"
            );
        }
        let dir = project.join("libraries");
        std::fs::create_dir_all(&dir)?;
        let (path, tablepath, tablekind, uri) = if symbols {
            (
                dir.join(format!("{name}.kicad_sym")),
                project.join("sym-lib-table"),
                "sym_lib_table",
                format!("${{KIPRJMOD}}/libraries/{name}.kicad_sym"),
            )
        } else {
            let item = root.value(1);
            ensure!(
                !item.is_empty() && !item.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']),
                "Invalid footprint name"
            );
            let pretty = dir.join(format!("{name}.pretty"));
            std::fs::create_dir_all(&pretty)?;
            (
                pretty.join(format!("{item}.kicad_mod")),
                project.join("fp-lib-table"),
                "fp_lib_table",
                format!("${{KIPRJMOD}}/libraries/{name}.pretty"),
            )
        };
        if path.exists() {
            let expected = p
                .get("expected_revision")
                .and_then(Value::as_str)
                .context("Existing library needs expected_revision from read_library")?;
            let previous = jenga_core::sexpr::parse(&std::fs::read_to_string(&path)?)?;
            ensure!(
                journal::revision_text(&previous.encoded()) == expected,
                "Library revision conflict"
            );
        }
        if symbols {
            let mut names = std::collections::HashSet::new();
            for symbol in root.children("symbol") {
                ensure!(
                    !symbol.value(1).is_empty() && names.insert(symbol.value(1)),
                    "Empty or duplicate symbol name"
                );
            }
        }
        let mut table = if tablepath.exists() {
            jenga_core::sexpr::parse(&std::fs::read_to_string(&tablepath)?)?
        } else {
            Node::list(tablekind, vec![Node::list("version", vec![Node::atom(7)])])
        };
        ensure!(
            table.name() == tablekind,
            "Invalid existing symbol library table"
        );
        let registered = table
            .children("lib")
            .find(|n| n.child("name").is_some_and(|n| n.value(1) == name));
        if let Some(existing) = registered {
            ensure!(
                existing.child("uri").is_some_and(|n| n.value(1) == uri),
                "Library name already registered elsewhere"
            );
        }
        if registered.is_none() {
            table.items_mut().push(Node::list(
                "lib",
                vec![
                    Node::list("name", vec![Node::string(name)]),
                    Node::list("type", vec![Node::string("KiCad")]),
                    Node::list("uri", vec![Node::string(&uri)]),
                    Node::list("options", vec![Node::string("")]),
                    Node::list(
                        "descr",
                        vec![Node::string(
                            p.get("description")
                                .and_then(Value::as_str)
                                .unwrap_or("Imported library"),
                        )],
                    ),
                ],
            ));
        }
        let previous = std::fs::read_to_string(&path).unwrap_or_default();
        jenga_core::document::atomic_write(&path, source.as_bytes(), true)?;
        jenga_core::document::atomic_write(&tablepath, table.document_text().as_bytes(), true)?;
        let previous_provenance: Value =
            std::fs::read_to_string(path.with_extension("provenance.json"))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null);
        let provenance = json!({"source":p.get("url").or_else(||p.get("path")).or_else(||previous_provenance.get("source")).cloned().unwrap_or(json!("authored via tools")),"license":p.get("license").or_else(||previous_provenance.get("license")).unwrap_or(&Value::Null),"attribution":p.get("attribution").or_else(||previous_provenance.get("attribution")).unwrap_or(&Value::Null),"imported_at_ms":journal::now()});
        jenga_core::document::atomic_write(
            &path.with_extension("provenance.json"),
            serde_json::to_string_pretty(&provenance)?.as_bytes(),
            false,
        )?;
        self.libraries = None;
        journal::record_resource(
            self.document()?,
            &path.to_string_lossy(),
            &previous,
            &source,
            p.get("actor").and_then(Value::as_str).unwrap_or("external"),
            "write_library",
        )?;
        Ok(
            json!({"path":path,"library":name,"registered":true,"revision":journal::revision_text(&root.encoded())}),
        )
    }
}
pub fn files(folder: &Path, depth: usize) -> Result<Vec<PathBuf>> {
    ensure!(depth < 32, "Folder depth exceeded");
    let mut out = vec![];
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let p = entry.path();
        let ty = entry.file_type()?;
        if ty.is_symlink() {
            continue;
        }
        if ty.is_dir()
            && !matches!(
                entry.file_name().to_str(),
                Some(".git" | ".jenga" | "target" | "libraries")
            )
        {
            out.extend(files(&p, depth + 1)?);
        } else if p
            .extension()
            .is_some_and(|e| e == "kicad_sch" || e == "kicad_pro")
        {
            out.push(p);
        }
        ensure!(out.len() <= 10000, "Too many project files");
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_are_atomic_and_revision_guarded() {
        let mut d = Document::new();
        let old = d.root.clone();
        let mut libs = Libraries::discover(&[], None);
        assert!(
            edit(
                &mut d,
                &mut libs,
                &json!({"actions":[{"op":"text","text":"Hello","at":[30,40]},{"op":"bad"}]})
            )
            .is_err()
        );
        assert_eq!(d.root, old);
        assert!(
            edit(
                &mut d,
                &mut libs,
                &json!({"expected_revision":"stale","actions":[]})
            )
            .is_err()
        );
        edit(&mut d,&mut libs,&json!({"actions":[{"op":"title_block","fields":{"title":"Test","comments":["Preserved"]}},{"op":"text","text":"Hello","at":[30,40]}]})).unwrap();
        assert_eq!(TitleBlock::read(&d).title, "Test");
        assert_eq!(inspect(&d)["objects"].as_array().unwrap().len(), 1);
    }
}
