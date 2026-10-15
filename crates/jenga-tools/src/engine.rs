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
    let mut failures = vec![];
    for (index, action) in actions.iter().enumerate() {
        crate::operations::checkpoint()?;
        let prior = candidate.clone();
        match action_apply(&mut candidate, libs, action) {
            Ok(mut value) => {
                value["action_index"] = json!(index);
                results.push(value);
            }
            Err(e) => {
                if request["dry_run"] != true {
                    return Err(e).with_context(|| {
                        format!(
                            "Action {index} ({}) failed; from={} to={}; no edits committed",
                            action["op"], action["from"], action["to"]
                        )
                    });
                }
                candidate = prior;
                failures.push(json!({"index":index,"action":action,"error":format!("{e:#}")}));
            }
        }
    }
    if request.get("expected_connections").is_some()
        || request.get("forbidden_connections").is_some()
    {
        let verification = crate::net_guard::verify(&candidate, request)?;
        if verification["passed"] != true {
            if request["dry_run"] != true {
                bail!("Connection intent failed: {}", verification["findings"]);
            }
            failures.push(json!({"kind":"connection_intent","findings":verification["findings"]}));
        }
    }
    if request.get("allow_named_net_merges") != Some(&Value::Bool(true))
        && let Err(e) = crate::net_guard::named_merges(doc, &candidate)
    {
        if request["dry_run"] != true {
            return Err(e);
        }
        failures.push(json!({"kind":"unintended_short","error":e.to_string()}));
    }
    if request["dry_run"] == true {
        failures.extend(crate::net_guard::unintended_merges(
            doc, &candidate, request,
        )?);
        return crate::workflows::store_preview(
            doc,
            &candidate,
            json!({"results":results,"failures":failures,"can_commit":failures.is_empty(),"verification":crate::workflows::verify(&candidate,request)?}),
        );
    }
    *doc = candidate;
    Ok(json!({"revision":journal::revision(doc),"results":results,"dirty":doc.dirty}))
}
fn action_apply(doc: &mut Document, libs: &mut Libraries, a: &Value) -> Result<Value> {
    let op = string(a, "op")?;
    match op {
        "cleanup" => return crate::workflows::cleanup(doc),
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
            let before = doc.clone();
            let scene = Scene::from_document(doc);
            let from = pin(doc, &scene, a.get("from").context("Missing from pin")?)?;
            let to = pin(doc, &scene, a.get("to").context("Missing to pin")?)?;
            let horizontal = a
                .get("horizontal_first")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let route = crate::routing::route_options(doc, &scene, from, to, horizontal, a)?;
            for segment in route.windows(2) {
                doc.add_wire(segment[0], segment[1]);
            }
            crate::net_guard::route_change(
                &before,
                doc,
                a.get("from").unwrap(),
                a.get("to").unwrap(),
            )?;
            return Ok(
                json!({"route_mm":route,"junctions_added":false,"verify":"Inspect connectivity and crossings before saving"}),
            );
        }
        "arrange_symbols" => {
            let ids = a["uuids"]
                .as_array()
                .context("arrange_symbols needs uuids")?;
            ensure!(
                !ids.is_empty() && ids.len() <= 1000,
                "Select 1–1000 symbols"
            );
            let origin = point(a, "at")?;
            let requested_columns = a["columns"].as_u64().map(|n| n.clamp(1, 20) as usize);
            let scene = Scene::from_document(doc);
            let mut objects = vec![];
            for id in ids {
                let i = target(doc, &json!({"uuid":id}))?;
                ensure!(
                    doc.root.items()[i].name() == "symbol",
                    "Only symbols can be arranged"
                );
                ensure!(
                    !scene.pins.iter().filter(|p| p.object == i).any(|p| doc
                        .root
                        .children("wire")
                        .any(|w| {
                            let pts = points(w, "pts");
                            pts.len() == 2 && checks::on_segment(p.at, pts[0], pts[1])
                        })),
                    "Arrange before wiring; connected symbols cannot be moved automatically"
                );
                ensure!(!objects.contains(&i), "Duplicate symbol UUID");
                objects.push(i);
            }
            let mut width = 20_f64;
            let mut height = 20_f64;
            for i in &objects {
                if let Some((lo, hi)) = scene.object_bounds(*i) {
                    width = width.max(hi.0 - lo.0 + 12.);
                    height = height.max(hi.1 - lo.1 + 12.);
                }
            }
            let paper = export::paper_size(doc);
            let columns = requested_columns
                .unwrap_or_else(|| ((paper.0 - origin.0 - 12.54) / width).floor().max(1.) as usize)
                .min(objects.len())
                .max(1);
            ensure!(
                origin.0 >= 10.
                    && origin.1 >= 10.
                    && origin.0 + columns.min(objects.len()) as f64 * width <= paper.0 - 10.
                    && origin.1 + objects.len().div_ceil(columns) as f64 * height <= paper.1 - 25.,
                "Layout exceeds this page. Use fewer components or a larger sheet"
            );
            for (j, i) in objects.iter().enumerate() {
                let (lo, _) = scene.object_bounds(*i).context("Symbol has no geometry")?;
                doc.translate(
                    *i,
                    ((origin.0 + (j % columns) as f64 * width - lo.0) / 1.27).round() * 1.27,
                    ((origin.1 + (j / columns) as f64 * height - lo.1) / 1.27).round() * 1.27,
                );
            }
            let placed = Scene::from_document(doc);
            ensure!(
                objects.iter().all(
                    |i| placed.object_bounds(*i).is_some_and(|(lo, hi)| lo.0 >= 9.
                        && lo.1 >= 9.
                        && hi.0 <= paper.0 - 9.
                        && hi.1 <= paper.1 - 24.)
                ),
                "Grid-aligned layout exceeds page margins; choose fewer columns or a larger page"
            );
            return Ok(
                json!({"arranged":objects.len(),"columns":columns,"cell_mm":[width,height]}),
            );
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
            let i = doc.add_text(op, string(a, "text")?, at.0, at.1);
            crate::formatting::apply(&mut doc.root.items_mut()[i], a)?;
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
        "refresh_symbol" | "replace_symbol" => return crate::replacement::replace(doc, libs, a),
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
        "format_text" | "format_property" => {
            let i = target(doc, a)?;
            let n = &mut doc.root.items_mut()[i];
            if op == "format_property" {
                ensure!(n.name() == "symbol", "Select a symbol property");
                let key = string(a, "key")?;
                let field = n
                    .items_mut()
                    .iter_mut()
                    .find(|v| v.name() == "property" && v.value(1) == key)
                    .context("Property not found")?;
                crate::formatting::apply(field, a)?;
            } else {
                ensure!(
                    matches!(
                        n.name(),
                        "text" | "label" | "global_label" | "hierarchical_label"
                    ),
                    "Select text or a label"
                );
                crate::formatting::apply(n, a)?;
            }
            doc.dirty = true;
        }
        "annotate" => return crate::formatting::annotate(doc, a),
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
    pub board: Option<jenga_core::pcb::Board>,
    pub record_history: bool,
    pub active_kind: String,
    pub libraries: Option<Libraries>,
    pub workspace: PathBuf,
    pub undo: Vec<Document>,
    pub redo: Vec<Document>,
}
impl Engine {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            doc: None,
            board: None,
            record_history: true,
            active_kind: "schematic".into(),
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
    fn library_sources(&mut self) {
        if self.libraries.is_none() {
            let project = self
                .doc
                .as_ref()
                .and_then(|d| d.path.as_ref())
                .and_then(|p| p.parent())
                .unwrap_or(&self.workspace);
            self.libraries = Some(Libraries::discover(&[], Some(project)));
        }
    }
    pub fn load_libraries(&mut self) -> Result<()> {
        self.library_sources();
        if self
            .libraries
            .as_ref()
            .is_some_and(|l| l.entries.is_empty())
        {
            self.libraries.as_mut().unwrap().index()?;
        }
        Ok(())
    }
    pub fn request(&mut self, method: &str, p: &Value) -> Result<Value> {
        if method == "export_formats" {
            return Ok(
                json!({"schematic":{"file":["kicad_sch","pdf","svg","png","csv"]},"pcb":{"file":["native","pdf","png","bom","pos","step","glb","stl","vrml","brep","ply","dxf","ipc2581","ipcd356","gencad"],"folder":["manufacturing","gerbers","drill","svg"]},"assembly":{"file":["png","pdf","step","glb","stl","vrml","brep","ply"],"views":["current","isometric","top","bottom","front","back","left","right"],"lighting":{"brightness":[0.25,4.0],"ambient":[0.1,1.0]}},"note":"Available CAD backends and individual formats can reject unsupported geometry; exports never change project save location"}),
            );
        }
        if method == "operations" {
            return crate::operations::list();
        }
        if method == "operation_cancel" {
            return crate::operations::cancel(string(p, "operation_id")?);
        }
        if method == "capabilities" {
            return Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"pid":std::process::id(),"engineering_protocol":1,"tools":crate::protocol::tools(),"background_previews":true,"connection":"Stable bridge reconnects without replaying interrupted edits"}),
            );
        }
        if method == "operation_status" {
            return crate::operations::status(string(p, "operation_id")?);
        }
        if p["background"] == true {
            ensure!(
                matches!(
                    method,
                    "pcb_preview"
                        | "pcb_autoroute"
                        | "pcb_autoplace"
                        | "build_pcb_from_schematic"
                        | "pcb_verify"
                        | "pcb_3d"
                        | "pcb_export"
                        | "pcb_export_3d_view"
                        | "preview_edit"
                ),
                "Background calls must calculate previews, verification or exports; commit/save explicitly afterward"
            );
            let mut params = p.clone();
            params["background"] = json!(false);
            if matches!(
                method,
                "pcb_autoroute" | "pcb_autoplace" | "build_pcb_from_schematic"
            ) {
                params["dry_run"] = json!(true);
            }
            let id = p["operation_id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            if let Some(receipt) = crate::operations::begin(&id, method, p)? {
                return Self::operation_response(receipt);
            }
            let slot = match crate::operations::acquire() {
                Ok(slot) => slot,
                Err(error) => {
                    let result: Result<Value> = Err(error);
                    return Self::operation_response(crate::operations::finish(&id, &result)?);
                }
            };
            let mut worker = Engine::new(self.workspace.clone());
            worker.doc = self.doc.clone();
            worker.board = self.board.as_ref().map(jenga_core::pcb::Board::snapshot);
            worker.active_kind = self.active_kind.clone();
            let operation = id.clone();
            let method = method.to_owned();
            std::thread::spawn(move || {
                let _slot = slot;
                crate::operations::context(
                    &operation,
                    params["timeout_ms"].as_u64().unwrap_or(120_000),
                );
                crate::operations::progress(&method, 0);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||worker.execute_staged(&method,&params))).unwrap_or_else(|_|Err(anyhow::anyhow!("Background worker panicked; no snapshot applied. Inspect operation and saved files before retrying.")));
                let _ = crate::operations::finish(&operation, &result);
            });
            return Ok(
                json!({"operation_id":id,"status":"pending","applied":false,"saved":false,"message":"Background snapshot accepted. Inspect status, review preview and commit at an unchanged revision."}),
            );
        }
        if crate::operations::mutation(method) {
            let id = p["operation_id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            if let Some(receipt) = crate::operations::begin(&id, method, p)? {
                return Self::operation_response(receipt);
            }
            let result = self.request_tracked(method, p);
            let receipt = crate::operations::finish(&id, &result)?;
            return Self::operation_response(receipt);
        }
        self.request_tracked(method, p)
    }
    pub fn operation_response(receipt: Value) -> Result<Value> {
        if receipt["status"] == "failed" {
            bail!(
                "Operation {} failed: {}",
                receipt["operation_id"],
                receipt["error"]
            );
        }
        let mut value = receipt["result"]
            .as_object()
            .cloned()
            .map(Value::Object)
            .unwrap_or_else(|| json!({}));
        value["operation_id"] = receipt["operation_id"].clone();
        value["status"] = receipt["status"].clone();
        Ok(value)
    }
    pub fn execute_staged(&mut self, method: &str, p: &Value) -> Result<Value> {
        self.record_history = false;
        self.request_tracked(method, p)
    }
    fn request_tracked(&mut self, method: &str, p: &Value) -> Result<Value> {
        if matches!(
            method,
            "import_project" | "move_project" | "import_document"
        ) {
            return crate::transfer::request(self, method, p);
        }
        if method == "engineering_review" {
            return crate::engineering::request(self, p);
        }
        if method == "import_asset" {
            return crate::engineering::import_asset(self, p);
        }
        if matches!(method, "copy_objects" | "paste_objects")
            && (p["target"] == "pcb" || (p["target"].is_null() && self.active_kind == "pcb"))
        {
            let b = self.board.as_mut().context("Open a PCB first")?;
            if let Some(rev) = p["expected_revision"].as_str() {
                ensure!(b.revision() == rev, "PCB revision conflict; inspect again");
            }
            if method == "copy_objects" {
                let ids = p["uuids"].as_array().map(|v| {
                    v.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                });
                return Ok(
                    json!({"text":jenga_core::pcb_clipboard::copy(b,ids.as_deref())?,"format":"kicad_pcb"}),
                );
            }
            let before = b.snapshot();
            if p["dry_run"] == true {
                let mut preview = b.snapshot();
                let mut result = jenga_core::pcb_clipboard::paste(
                    &mut preview,
                    string(p, "text")?,
                    if p["at"].is_array() {
                        Some(point(p, "at")?)
                    } else {
                        None
                    },
                    true,
                )?;
                result["applied"] = json!(false);
                result["saved"] = json!(false);
                result["revision"] = json!(b.revision());
                return Ok(result);
            }
            let result = jenga_core::pcb_clipboard::paste(
                b,
                string(p, "text")?,
                if p["at"].is_array() {
                    Some(point(p, "at")?)
                } else {
                    None
                },
                p["allow_named_net_merges"] == true,
            )?;
            b.undo.push(before.state());
            b.redo.clear();
            return Ok(result);
        }
        if let Some(target) = p["target"].as_str() {
            ensure!(
                matches!(target, "schematic" | "pcb"),
                "target must be schematic or pcb"
            );
        }
        if p["target"] == "pcb" || (p["target"].is_null() && self.active_kind == "pcb") {
            let mapped = match method {
                "inspect" => Some("pcb_inspect"),
                "save" => Some("pcb_save"),
                "verify" | "erc" => Some("pcb_verify"),
                "undo" => Some("pcb_undo"),
                "redo" => Some("pcb_redo"),
                "history" => Some("pcb_history"),
                "export" => Some("pcb_export"),
                "close" => Some("pcb_close"),
                _ => None,
            };
            if let Some(mapped) = mapped {
                return crate::pcb_tools::request(self, mapped, p);
            }
        }

        if crate::pcb_tools::METHODS.contains(&method) {
            return crate::pcb_tools::request(self, method, p);
        }
        if method == "list_archived_projects" {
            return Ok(crate::projects::archived_projects());
        }
        if method == "restore_project" {
            return crate::projects::restore_project(p);
        }
        if method == "connect_block" {
            return self.request_tracked("edit", &crate::workflows::block(p)?);
        }
        if method == "preview_edit" {
            let mut request = p.clone();
            request["dry_run"] = json!(true);
            return self.request_tracked("edit", &request);
        }
        if method == "verify" {
            return crate::workflows::verify(self.document()?, p);
        }
        if method == "commit_preview" {
            let path = crate::workflows::preview_path(string(p, "preview_id")?)?;
            let receipt: Value =
                serde_json::from_slice(&std::fs::read(path).context("Unknown preview_id")?)?;
            ensure!(
                receipt["result"]["can_commit"] == true,
                "Preview has routing or connectivity failures; fix them and preview again"
            );
            let before = self.document()?.clone();
            if let Some(expected) = p["expected_revision"].as_str() {
                ensure!(
                    expected == journal::revision(&before),
                    "Revision conflict; preview again"
                );
            }
            ensure!(
                receipt["base_revision"] == journal::revision(&before)
                    && receipt["root_uuid"] == before.root_uuid()
                    && receipt["file"] == json!(before.path),
                "Preview is stale or belongs to another document; preview again"
            );
            ensure!(
                !before.disk_changed()?,
                "Saved document changed externally; reopen and preview again"
            );
            let mut after = before.clone();
            after.root = jenga_core::sexpr::parse(string(&receipt, "source")?)?;
            after.dirty = true;
            journal::record(
                &before,
                &after,
                p["actor"].as_str().unwrap_or("external"),
                "commit_preview",
            )?;
            self.undo.push(before);
            self.redo.clear();
            self.doc = Some(after);
            return Ok(
                json!({"revision":journal::revision(self.document()?),"dirty":true,"applied":true,"preview_id":p["preview_id"]}),
            );
        }
        let old = self.doc.as_ref().and_then(|d| d.path.clone());
        let old_board = self.board.as_ref().and_then(|b| b.path.clone());
        if matches!(method, "rename" | "save_project_as" | "delete_project") {
            ensure!(
                !self.board.as_ref().is_some_and(|b| b.dirty),
                "Save the PCB before project maintenance"
            );
        }
        let mut result = self.request_inner(method, p);
        if result.is_ok()
            && matches!(method, "rename" | "save_project_as")
            && let (Some(old_board), Some(new_doc)) =
                (old_board, self.doc.as_ref().and_then(|d| d.path.as_ref()))
        {
            let folder = new_doc.parent().unwrap();
            let paired = folder.join(format!(
                "{}.kicad_pcb",
                new_doc.file_stem().unwrap_or_default().to_string_lossy()
            ));
            let same = folder.join(old_board.file_name().unwrap());
            let path = if paired.is_file() { paired } else { same };
            if path.is_file() {
                self.board = Some(jenga_core::pcb::Board::open(&path)?);
            }
        }
        if result.is_ok()
            && matches!(
                method,
                "open"
                    | "create_project"
                    | "create_schematic"
                    | "create_page"
                    | "delete_page"
                    | "rename_page"
                    | "reorder_pages"
                    | "rename"
                    | "save_project_as"
            )
            && let Some(doc) = self.doc.as_mut()
            && let Some(path) = &doc.path
            && let Ok(project) = crate::project_workspace::discover(path)
        {
            doc.page_context = project.page_context(path);
        }
        if result.is_ok()
            && matches!(
                method,
                "open"
                    | "save"
                    | "create_project"
                    | "create_schematic"
                    | "rename"
                    | "save_project_as"
                    | "delete_project"
            )
        {
            let path = self.doc.as_ref().and_then(|d| d.path.as_deref());
            if let Err(e) = crate::catalog::record(
                method,
                if matches!(method, "rename" | "delete_project") {
                    old.as_deref()
                } else {
                    None
                },
                path,
            ) && let Ok(value) = &mut result
            {
                value["catalog_warning"] = json!(e.to_string());
            }
        }
        result
    }
    fn request_inner(&mut self, method: &str, p: &Value) -> Result<Value> {
        match method {
            "copy_symbol" => {
                self.load_libraries()?;
                let def = self.libraries.as_mut().unwrap().symbol(string(p, "id")?)?;
                let mut params = p.clone();
                params["name"] = json!(def.value(1));
                params["native_source"] = json!(def.encoded());
                if !params["scope"].is_string() {
                    params["scope"] = json!("personal");
                }
                if !params["license"].is_string() {
                    params["license"] =
                        json!("Original source library license applies; not relicensed");
                }
                if !params["attribution"].is_string() {
                    params["attribution"] = json!(format!("Copied from {}", string(p, "id")?));
                }
                self.request_inner("save_symbol", &params)
            }
            "trace_connection" => crate::net_guard::trace(self.document()?, p),
            "diagnose" => Ok(
                json!({"revision":journal::revision(self.document()?),"file":self.document()?.path,"dirty":self.document()?.dirty,"erc":{"issues":findings(self.document()?)},"connectivity":jenga_core::connectivity::inspect(self.document()?),"next":"Inspect named nets; use trace_connection and verify_connections. Save before comparing a new headless session."}),
            ),
            "manage_symbol" => {
                let result = crate::library_manage::manage(self.doc.as_ref(), p)?;
                self.libraries = None;
                Ok(result)
            }
            "delete_page" => {
                let result = crate::project_workspace::delete_page(self.document()?, p)?;
                if self
                    .doc
                    .as_ref()
                    .and_then(|d| d.path.as_ref())
                    .is_some_and(|path| {
                        result["deleted"].as_str() == Some(path.to_string_lossy().as_ref())
                    })
                {
                    self.doc = result["open_after"]
                        .as_str()
                        .map(|p| Document::open(std::path::Path::new(p)))
                        .transpose()?;
                } else if let Some(path) = self.doc.as_ref().and_then(|d| d.path.clone()) {
                    self.doc = Some(Document::open(&path)?);
                }
                Ok(result)
            }
            "rename_page" | "reorder_pages" => {
                let result = if method == "rename_page" {
                    crate::project_workspace::rename_page(self.document()?, p)?
                } else {
                    crate::project_workspace::reorder_pages(self.document()?, p)?
                };
                let path = self.document()?.path.clone().unwrap();
                self.doc = Some(Document::open(&path)?);
                Ok(result)
            }
            "verify_connections" => crate::net_guard::verify(self.document()?, p),
            "save_symbol" => {
                if let Some(fp) = p
                    .get("footprint")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    self.load_libraries()?;
                    self.libraries
                        .as_ref()
                        .unwrap()
                        .footprint(fp)
                        .context("Selected footprint cannot be resolved")?;
                }
                let symbol = crate::symbol_editor::symbol(p)?;
                let library = string(p, "library")?;
                crate::projects::valid_name(library)?;
                let base = if p.get("scope").and_then(Value::as_str) == Some("personal") {
                    crate::symbol_editor::personal_folder()?
                } else {
                    self.document()?
                        .path
                        .as_ref()
                        .and_then(|p| p.parent())
                        .context("Save project first")?
                        .to_owned()
                };
                let path = base.join("libraries").join(format!("{library}.kicad_sym"));
                let mut root = if path.is_file() {
                    let expected = string(p, "expected_revision")?;
                    let root = jenga_core::sexpr::parse(&std::fs::read_to_string(&path)?)?;
                    ensure!(
                        journal::revision_text(&root.encoded()) == expected,
                        "Library revision conflict"
                    );
                    root
                } else {
                    Node::list(
                        "kicad_symbol_lib",
                        vec![
                            Node::list("version", vec![Node::atom(20231120)]),
                            Node::list("generator", vec![Node::atom("jenga")]),
                        ],
                    )
                };
                root.items_mut()
                    .retain(|n| !(n.name() == "symbol" && n.value(1) == symbol.value(1)));
                root.items_mut().push(symbol);
                let mut params = p.clone();
                params["name"] = json!(library);
                params["source"] = json!(root.document_text());
                self.import(&params)
            }
            "project_info" => {
                let path = if let Some(path) = p.get("path").and_then(Value::as_str) {
                    self.resolve(path)
                } else {
                    self.board
                        .as_ref()
                        .filter(|_| self.active_kind == "pcb")
                        .and_then(|b| b.path.clone())
                        .or_else(|| self.doc.as_ref()?.path.clone())
                        .context("Open a saved project first")?
                };
                let project = crate::project_workspace::discover(&path)?;
                let mut info = json!(project);
                for page in info["pages"].as_array_mut().unwrap() {
                    let linked = !page["parent"].is_null();
                    page["page_kind"] = json!(if linked {
                        "hierarchical_child"
                    } else {
                        "independent_drawing"
                    });
                    page["electrical_scope"] = json!(if linked {
                        "Linked through explicit hierarchical sheet pins; cross-sheet analysis requires KiCad"
                    } else {
                        "Separate electrical drawing; local labels do not connect other roots"
                    });
                }
                info["analysis_scope"] = json!("single_sheet");
                Ok(info)
            }
            "create_page" => {
                if self.doc.is_none() {
                    ensure!(
                        p["hierarchical"] != true,
                        "A linked child requires a parent schematic"
                    );
                    let board = self.board.as_ref().context("Open a project first")?;
                    let folder = board
                        .path
                        .as_ref()
                        .and_then(|p| p.parent())
                        .context("Save the PCB project first")?;
                    let name = string(p, "name")?;
                    crate::projects::valid_name(name)?;
                    let path = folder.join(format!("{name}.kicad_sch"));
                    ensure!(!path.exists(), "Schematic already exists");
                    let mut d = Document::new();
                    let mut title = jenga_core::page::TitleBlock::read(&d);
                    title.title = name.into();
                    title.write(&mut d);
                    d.save(&path)?;
                    self.doc = Some(d);
                    self.active_kind = "schematic".into();
                    return Ok(json!({"file":path,"hierarchical":false,"project_kind":"mixed"}));
                }
                if let Some(expected) = p.get("expected_revision").and_then(Value::as_str) {
                    ensure!(
                        expected == journal::revision(self.document()?),
                        "Revision conflict: inspect again"
                    );
                }
                let before = self.document()?.clone();
                let result = crate::project_workspace::create_page(
                    self.doc.as_mut().context("Open a project first")?,
                    p,
                )?;
                journal::record(
                    &before,
                    self.document()?,
                    p.get("actor").and_then(Value::as_str).unwrap_or("external"),
                    "create_page",
                )?;
                Ok(result)
            }
            "export_pages" => {
                ensure!(
                    !self.document()?.dirty,
                    "Save before exporting project pages"
                );
                let path = self
                    .document()?
                    .path
                    .as_ref()
                    .context("Save project first")?;
                let project = crate::project_workspace::discover(path)?;
                crate::project_workspace::export_pages(&project, p)
            }
            "help" => {
                crate::help::get(p.get("topic").and_then(Value::as_str).unwrap_or("overview"))
            }
            "open" => {
                ensure!(
                    !self.doc.as_ref().is_some_and(|d| d.dirty)
                        || p.get("discard_changes") == Some(&Value::Bool(true)),
                    "Unsaved changes: save first or explicitly discard_changes"
                );
                let mut path = self.resolve(string(p, "path")?);
                if path.extension().is_some_and(|e| e == "kicad_pro") {
                    path.set_extension(if path.with_extension("kicad_sch").is_file() {
                        "kicad_sch"
                    } else {
                        "kicad_pcb"
                    });
                }
                if path.extension().is_some_and(|e| e == "kicad_pcb") {
                    let mut q = p.clone();
                    q["path"] = json!(path);
                    return crate::pcb_tools::request(self, "pcb_open", &q);
                }
                let next = Document::open(&path)?;
                let different_project = self
                    .board
                    .as_ref()
                    .is_some_and(|b| b.path.as_ref().and_then(|p| p.parent()) != path.parent());
                if different_project {
                    ensure!(
                        !self.board.as_ref().is_some_and(|b| b.dirty)
                            || p["discard_changes"] == true,
                        "PCB has unsaved changes; save before opening another project"
                    );
                    self.board = None;
                }
                self.doc = Some(next);
                self.active_kind = "schematic".into();
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
                let name = string(p, "name")?;
                let parent = if folder
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
                {
                    folder
                        .parent()
                        .context("Missing destination parent")?
                        .to_owned()
                } else {
                    folder.clone()
                };
                self.doc = Some(create_project(&parent, name)?);
                self.undo.clear();
                self.redo.clear();
                self.libraries = None;
                Ok(inspect(self.document()?))
            }
            "view_skill" => Ok(
                json!({"name":"jenga-design","version":env!("CARGO_PKG_VERSION"),"text":if p["topic"]=="engineering"{crate::setup::ENGINEERING_SKILL}else{crate::setup::SKILL},"references":{"engineering":"Call view_skill with topic: engineering for evidence, labeling and asset acquisition guidance"}}),
            ),
            "export_skill" => {
                let path = self.resolve(string(p, "path")?);
                crate::setup::export_skill(&path)
            }
            "copy_objects" => {
                let uuids = p
                    .get("uuids")
                    .map(|v| serde_json::from_value::<Vec<String>>(v.clone()))
                    .transpose()?;
                let text = jenga_core::clipboard::copy(self.document()?, uuids.as_deref())?;
                Ok(json!({"text":text,"format":"kicad_clipboard"}))
            }
            "paste_objects" => {
                if self.document()?.disk_changed()? {
                    ensure!(
                        !self.document()?.dirty,
                        "External changes conflict with unsaved edits"
                    );
                    let path = self.document()?.path.clone().unwrap();
                    self.doc = Some(Document::open(&path)?);
                }
                let before = self.document()?.clone();
                if let Some(expected) = p.get("expected_revision").and_then(Value::as_str) {
                    ensure!(
                        expected == journal::revision(&before),
                        "Revision conflict: inspect again"
                    );
                }
                let at = p.get("at").map(|_| point(p, "at")).transpose()?;
                let ids = jenga_core::clipboard::paste(
                    self.doc.as_mut().unwrap(),
                    string(p, "text")?,
                    at,
                )?;
                if let Err(e) = journal::record(
                    &before,
                    self.document()?,
                    p.get("actor").and_then(Value::as_str).unwrap_or("external"),
                    "paste_objects",
                ) {
                    self.doc = Some(before);
                    return Err(e);
                }
                self.undo.push(before);
                self.redo.clear();
                Ok(json!({"uuids":ids,"revision":journal::revision(self.document()?)}))
            }
            "ui_state" => Ok(
                json!({"running_app":false,"backend":"headless","workspace":self.workspace,"document":self.doc.as_ref().map(inspect)}),
            ),
            "inspect" if self.doc.is_none() && self.board.is_some() => {
                crate::pcb_tools::request(self, "pcb_inspect", p)
            }
            "inspect" => Ok(if let Some(doc) = &self.doc {
                crate::workflows::inspect(doc, p)?
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
                self.library_sources();
                let id = string(p, "id")?;
                let def = self.libraries.as_mut().unwrap().symbol(id)?;
                let scene = Scene::symbol_preview_unit(
                    &def,
                    p.get("unit").and_then(Value::as_u64).unwrap_or(1) as u32,
                );
                Ok(
                    json!({"id":id,"definition":def.encoded(),"bounds_mm":scene.content_bounds(),"physical_pin_count":def.children("symbol").flat_map(|s|s.children("pin")).filter_map(|p|p.child("number").map(|n|n.value(1))).collect::<std::collections::HashSet<_>>().len(),"default_footprint":def.children("property").find(|p|p.value(1)=="Footprint").map(|p|p.value(2)),"pins":scene.pins.iter().map(|p|json!({"number":p.number,"at_mm":p.at,"type":p.kind,"name":p.name,"outward":p.outward})).collect::<Vec<_>>()}),
                )
            }
            "get_footprint" => {
                if self.libraries.is_none() {
                    let folder = crate::footprint_tools::folder(self);
                    self.libraries = Some(Libraries::discover(&[], Some(&folder)));
                }
                let id = string(p, "id")?;
                let def = self.libraries.as_ref().unwrap().footprint(id)?;
                Ok(
                    json!({"id":id,"definition":def.encoded(),"bounds_mm":Scene::footprint_preview(&def).content_bounds(),"models_3d":def.children("model").map(|n|json!({"path":n.value(1),"native_transform":n.encoded()})).collect::<Vec<_>>(),"model_note":"Native footprint model links are preserved. A schematic has no physical board placement; 3D assembly rendering requires a PCB/model viewer."}),
                )
            }
            "edit" => {
                if self.document()?.disk_changed()? {
                    ensure!(
                        !self.document()?.dirty,
                        "External changes conflict with unsaved edits; save a separate copy or reopen explicitly"
                    );
                    let path = self.document()?.path.clone().unwrap();
                    self.doc = Some(Document::open(&path)?);
                }
                self.library_sources();
                let before = self.document()?.clone();
                let result = edit(
                    self.doc.as_mut().context("No document open")?,
                    self.libraries.as_mut().unwrap(),
                    p,
                )?;
                if p["dry_run"] == true {
                    return Ok(result);
                }
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
                    "xlsx" => export::bom_xlsx(doc, &path)?,
                    "kicad_sch" => export::schematic(doc, &path)?,
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
            "connectivity" => {
                let mut value = jenga_core::connectivity::inspect(self.document()?);
                value["revision"] = json!(journal::revision(self.document()?));
                Ok(value)
            }
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
            "save_project_as" => {
                let folder = self.resolve(string(p, "folder")?);
                let before = self.document()?.clone();
                let copied = crate::projects::copy_project(&before, &folder, string(p, "name")?)?;
                self.doc = Some(copied);
                self.libraries = None;
                self.undo.clear();
                self.redo.clear();
                Ok(inspect(self.document()?))
            }
            "delete_project" => {
                let result = crate::projects::archive_project(self.document()?)?;
                self.doc = None;
                self.undo.clear();
                self.redo.clear();
                self.libraries = None;
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
        let project = if p.get("scope").and_then(Value::as_str) == Some("personal") {
            crate::symbol_editor::personal_folder()?
        } else {
            p["project_folder"]
                .as_str()
                .map(|p| self.resolve(p))
                .or_else(|| {
                    self.board
                        .as_ref()
                        .and_then(|b| b.path.as_ref())
                        .and_then(|p| p.parent())
                        .map(Path::to_owned)
                })
                .or_else(|| {
                    self.doc
                        .as_ref()
                        .and_then(|d| d.path.as_ref())
                        .and_then(|p| p.parent())
                        .map(Path::to_owned)
                })
                .context("Save schematic before importing a project library")?
        };
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
        if let Some(doc) = self.doc.as_ref() {
            journal::record_resource(
                doc,
                &path.to_string_lossy(),
                &previous,
                &source,
                p.get("actor").and_then(Value::as_str).unwrap_or("external"),
                "write_library",
            )?;
        }
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
            .is_some_and(|e| e == "kicad_sch" || e == "kicad_pro" || e == "kicad_pcb")
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
