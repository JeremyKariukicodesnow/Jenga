//! PCB engineering workflows for humans, CLI and MCP. Edits are staged on a clone.
use crate::{
    engine::{Engine, string},
    footprint_tools, pcb_backend, pcb_routing,
};
use anyhow::{Context, Result, bail, ensure};
use jenga_core::{
    document::{Document, atomic_write, property},
    pcb::{self, Board},
    pcb_checks,
    scene::{Point, Scene},
    sexpr::{Node, parse},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

pub const METHODS: &[&str] = &[
    "pcb_export_3d_view",
    "pcb_create_page",
    "pcb_export_pages",
    "pcb_preflight",
    "pcb_update_from_schematic",
    "model_create",
    "footprint_3d",
    "pcb_new",
    "pcb_open",
    "pcb_close",
    "pcb_save",
    "pcb_inspect",
    "pcb_edit",
    "pcb_connect",
    "pcb_preview",
    "pcb_commit",
    "pcb_verify",
    "pcb_trace",
    "pcb_sync",
    "pcb_assign_footprints",
    "pcb_autoplace",
    "pcb_autoroute",
    "build_pcb_from_schematic",
    "pcb_fill_zones",
    "pcb_export",
    "pcb_3d",
    "pcb_undo",
    "pcb_redo",
    "pcb_history",
    "footprint_save",
    "footprint_manage",
    "footprint_copy",
    "model_import",
    "model_inspect",
    "model_export",
    "pcb_create_project",
    "pcb_copy_project",
    "pcb_rename_project",
    "pcb_archive_project",
];
pub fn mutating(method: &str) -> bool {
    !matches!(
        method,
        "pcb_inspect"
            | "pcb_verify"
            | "pcb_trace"
            | "pcb_history"
            | "model_inspect"
            | "pcb_preflight"
    ) && METHODS.contains(&method)
}
pub fn board(e: &Engine) -> Result<&Board> {
    e.board.as_ref().context("Open or create a PCB first")
}
pub fn description(method: &str) -> &str {
    match method {
        "pcb_export_3d_view" => {
            "Export native 3D assembly camera views as PNG or one multi-page PDF. view/ views: isometric, top, bottom, front, back, left, right, current; current accepts yaw/pitch/zoom. CAD model exports use pcb_export."
        }
        "pcb_create_page" => {
            "Create a named independent native PCB in the current project folder. Each PCB page is a separate physical board; refuses existing files and unsaved current-board edits."
        }
        "pcb_export_pages" => {
            "Export selected or all PCB pages: combined PDF, or PNG/SVG/native boards in a folder. pages accepts board names or full paths; native files remain separate physical boards."
        }
        "model_create" => {
            "Create a project-local native VRML component model from bounded box/cylinder/sphere shapes in millimetres, colors 0–1 and cylinder axes x/y/z. Attach returned reference to a stock or custom footprint; representative geometry must be checked against the real package."
        }
        "pcb_preflight" => {
            "Audit every schematic reference, footprint assignment and physical pin-to-pad map without editing. Returns compact repair findings before building or updating a PCB."
        }
        "pcb_update_from_schematic" => {
            "Preview a guarded existing-board update, preserving placement and routing. Includes footprint audit, changed pad nets and native DRC. dry_run defaults true; failures block commit."
        }
        "pcb_edit" | "pcb_preview" => {
            "Stage native PCB actions atomically: add_footprint, pad_net, track, via, outline, rules, stackup, move, rotate, flip, replace_footprint with pad_map, model, zone, keepout, text, delete, cleanup. All coordinates and clearances are mm. Preview returns changes, exact failures, unintended shorts, preview_id and image; commit only at unchanged revisions. Save explicitly."
        }
        "build_pcb_from_schematic" => {
            "Preview footprint validation, schematic-to-PCB net mapping, connectivity-aware placement and bounded clearance-aware multilayer routing together. footprints maps references to library IDs; pad_maps maps exact pin numbers to pads. Hierarchical designs use KiCad's native netlist. Incomplete routing is reported; dry_run:false applies only successful batches unless allow_partial:true."
        }
        "pcb_connect" | "pcb_autoroute" => {
            "Route same-net pads with exact reference or footprint uuid and number, avoiding other nets, floating pads, mechanical holes, board edges and keepouts. Atomic by default; dry_run previews. Bounded routing reports failed endpoint pairs and obstacles precisely."
        }
        "pcb_verify" => {
            "One verification call for native geometry, shorts, clearance, unrouted nets, widths, vias, board outline, expected_connections, forbidden_connections and missing 3D models. external:true runs native KiCad DRC for complete geometry/rules. Never infer manufacturing readiness from 3D rendering."
        }
        "pcb_export" => {
            "Export native KiCad PCB, PNG, SVG, PDF, BOM, placement CSV, Gerbers, drill, manufacturing bundle or STEP/GLB/STL/VRML/BREP/PLY. Manufacturing requires successful native DRC unless allow_unverified:true. Existing destinations are protected."
        }
        "footprint_save" => {
            "Create or safely update native .kicad_mod in project/personal library. native_source preserves all graphics and pad primitives; structured pads, graphics and models also accepted. Updates require expected_revision from read_library. Model transforms use mm/degrees."
        }
        "pcb_inspect" => {
            "Compact native PCB inspection by references, uuids, kinds, include_pads or since_revision. Includes net assignments, model links, board rules, electrical schematic link and revision."
        }
        "pcb_3d" => {
            "Generate cached native GLB assembly including board outline, holes, copper, zones and resolved footprint 3D models. Returns model findings and mesh bounds in mm. STEP/VRML models are converted by the local KiCad backend."
        }
        _ => {
            "Native PCB/footprint/3D engineering operation. Use help topic pcb for recipes and schemas. Mutations accept operation_id for exactly-once receipt lookup and expected_revision for guarded updates; no implicit PCB save."
        }
    }
}
pub fn schema(method: &str) -> Value {
    let mut props = json!({});
    for k in [
        "actor",
        "operation_id",
        "expected_revision",
        "scope",
        "new_name",
        "project_folder",
        "path",
        "output",
        "format",
        "name",
        "library",
        "id",
        "action",
        "native_source",
        "preview_id",
        "since_revision",
        "net",
        "layer",
        "view",
    ] {
        props[k] = json!({"type":"string"});
    }
    for k in [
        "dry_run",
        "external",
        "allow_partial",
        "allow_unverified",
        "include_pads",
        "personal",
        "discard_changes",
        "route",
        "autoplace",
        "link",
        "update_footprints",
        "allow_shorts",
        "include_schematic",
        "allow_rule_violations",
    ] {
        props[k] = json!({"type":"boolean"});
    }
    for k in [
        "width_mm",
        "height_mm",
        "width",
        "rotation",
        "track_width_mm",
        "height",
        "yaw",
        "pitch",
        "zoom",
    ] {
        props[k] = json!({"type":"number"});
    }
    for k in [
        "rules",
        "footprints",
        "pad_maps",
        "from",
        "to",
        "route_options",
    ] {
        props[k] = json!({"type":"object"});
    }
    for k in [
        "actions",
        "shapes",
        "connections",
        "expected_connections",
        "forbidden_connections",
        "pads",
        "graphics",
        "models",
        "reserved_corridors",
        "groups",
    ] {
        props[k] = json!({"type":"array","items":{"type":"object"}});
    }
    for k in ["references", "uuids", "kinds", "layers", "views", "pages"] {
        props[k] = json!({"type":"array","items":{"type":"string"}});
    }
    props["folder"] = json!({"type":"string"});
    let required = match method {
        "pcb_create_page" => vec!["name"],
        "pcb_export_3d_view" | "pcb_export_pages" => vec!["path"],
        "model_create" => vec!["name", "shapes"],
        "pcb_open" | "pcb_export" | "model_import" | "model_inspect" | "model_export" => {
            vec!["path"]
        }
        "pcb_commit" => vec!["preview_id"],
        "pcb_connect" => vec!["connections"],
        "pcb_create_project" | "pcb_copy_project" => vec!["name", "folder"],
        "footprint_save" => vec!["name", "library"],
        "footprint_copy" => vec!["id", "library"],
        "footprint_manage" => vec!["name", "library", "action"],
        _ => vec![],
    };
    json!({"type":"object","properties":props,"required":required,"additionalProperties":true})
}
pub fn request(e: &mut Engine, method: &str, p: &Value) -> Result<Value> {
    match method {
        "pcb_export_3d_view" => crate::assembly_view::export(e, p),
        "pcb_create_page" => {
            let name = string(p, "name")?;
            crate::projects::valid_name(name)?;
            ensure!(
                !e.board.as_ref().is_some_and(|b| b.dirty),
                "Save the current PCB before creating a page"
            );
            let folder = footprint_tools::folder(e);
            let path = folder.join(format!("{name}.kicad_pcb"));
            ensure!(
                !path.exists(),
                "PCB page already exists: {}",
                path.display()
            );
            let mut b = Board::new(
                p["width_mm"].as_f64().unwrap_or(80.),
                p["height_mm"].as_f64().unwrap_or(60.),
            )?;
            b.save(&path)?;
            e.board = Some(b);
            e.active_kind = "pcb".into();
            Ok(
                json!({"file":path,"name":name,"page_semantics":"independent_physical_board","linked_schematic":false}),
            )
        }
        "pcb_export_pages" => export_pages(e, p),
        "model_create" => crate::model_create::create(e, p),
        "pcb_preflight" => {
            let doc = e
                .doc
                .as_ref()
                .context("Open the intended schematic before auditing footprints")?;
            let libs = jenga_libraries::Libraries::discover(&[], Some(&footprint_tools::folder(e)));
            preflight(doc, p, &libs)
        }
        "pcb_update_from_schematic" => {
            let b = board(e)?;
            let doc = e
                .doc
                .as_ref()
                .context("Open the intended schematic before updating this board")?;
            if let Some(link) = &b.schematic {
                ensure!(
                    doc.path.as_ref() == Some(link),
                    "This PCB links to {}. Open that schematic before updating; no edits applied",
                    link.display()
                );
            }
            let mut q = p.clone();
            if q["dry_run"].is_null() {
                q["dry_run"] = json!(true);
            }
            q["external"] = json!(true);
            if q["autoplace"].is_null() {
                q["autoplace"] = json!(true);
            }
            transaction(e, "pcb_sync", &q)
        }
        "footprint_3d" => {
            let folder = p["project_folder"]
                .as_str()
                .map(|p| e.resolve(p))
                .unwrap_or_else(|| footprint_tools::folder(e));
            let def = if p["native_source"].is_string() {
                pcb::footprint_source(p)?
            } else {
                jenga_libraries::Libraries::discover(&[], Some(&folder))
                    .footprint(string(p, "id")?)?
            };
            let path = crate::catalog::directory()
                .join("footprint-3d")
                .join(format!("{}.glb", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(path.parent().unwrap())?;
            let backend = pcb_backend::footprint_mesh(&def, &folder, &path, true)?;
            let mesh = jenga_core::mesh::Mesh::read(&path)?;
            Ok(json!({"path":path,"mesh":mesh.info(),"backend":backend}))
        }
        "footprint_save" => {
            let v = footprint_tools::save(e, p)?;
            e.libraries = None;
            Ok(v)
        }
        "footprint_manage" => {
            let v = footprint_tools::manage(e, p)?;
            e.libraries = None;
            Ok(v)
        }
        "footprint_copy" => footprint_tools::copy(e, p),
        "model_import" => footprint_tools::import_model(e, p),
        "model_inspect" => {
            let path = e.resolve(string(p, "path")?);
            ensure!(path.is_file(), "3D model not found: {}", path.display());
            let mut info = if ["glb", "stl", "obj"]
                .iter()
                .any(|s| path.extension().is_some_and(|e| e == *s))
            {
                jenga_core::mesh::Mesh::read(&path)?.info()
            } else {
                let temp = tempfile::tempdir()?;
                let def = pcb::footprint_source(
                    &json!({"name":"ModelPreview","pads":[{"number":"1","kind":"smd","at":[0,0],"size":[1,1]}],"models":[{"path":path}]}),
                )?;
                let out = temp.path().join("model.glb");
                pcb_backend::footprint_mesh(&def, path.parent().unwrap(), &out, false)?;
                jenga_core::mesh::Mesh::read(&out)?.info()
            };
            info["path"] = json!(path);
            Ok(info)
        }
        "model_export" => {
            let input = e.resolve(string(p, "path")?);
            let output = e.resolve(string(p, "output")?);
            ensure!(!output.exists(), "Model export destination exists");
            let format = p["format"]
                .as_str()
                .or_else(|| output.extension().and_then(|e| e.to_str()))
                .unwrap_or("stl");
            if ["stl", "obj"].contains(&format)
                && ["glb", "stl", "obj"]
                    .iter()
                    .any(|s| input.extension().is_some_and(|e| e == *s))
            {
                let mesh = jenga_core::mesh::Mesh::read(&input)?;
                if format == "stl" {
                    mesh.export_stl(&output)?;
                } else {
                    mesh.export_obj(&output)?;
                }
                return Ok(json!({"path":output,"format":format,"mesh":mesh.info()}));
            }
            let mut b = Board::new(20., 20.)?;
            let def = pcb::footprint_source(
                &json!({"name":"ExportModel","pads":[{"number":"1","at":[0,0],"size":[1,1]}],"models":[{"path":input}]}),
            )?;
            b.add_footprint("Models:Export", &def, "M1", "", (20., 20.), 0.)?;
            let temp = tempfile::tempdir()?;
            let native = pcb_backend::prepared(&b, temp.path())?;
            let args = vec![
                "pcb".into(),
                "export".into(),
                format.into(),
                "--no-board-body".into(),
                "--force".into(),
                "--subst-models".into(),
                "--output".into(),
                output.to_string_lossy().into(),
                native.to_string_lossy().into(),
            ];
            let backend = pcb_backend::run(&args, Duration::from_secs(120))?;
            Ok(json!({"path":output,"format":format,"backend":backend}))
        }
        "pcb_create_project" => {
            let name = string(p, "name")?;
            crate::projects::valid_name(name)?;
            let parent = e.resolve(string(p, "folder")?);
            let folder = if parent.file_name().is_some_and(|n| n == name) {
                parent
            } else {
                parent.join(name)
            };
            ensure!(
                !folder.exists(),
                "Project destination exists: {}",
                folder.display()
            );
            std::fs::create_dir_all(&folder)?;
            let result = (|| -> Result<Value> {
                let path = folder.join(format!("{name}.kicad_pcb"));
                let mut b = Board::new(
                    p["width_mm"].as_f64().unwrap_or(80.),
                    p["height_mm"].as_f64().unwrap_or(60.),
                )?;
                atomic_write(&path.with_extension("kicad_pro"),serde_json::to_string_pretty(&json!({"meta":{"filename":format!("{name}.kicad_pro"),"version":1},"board":{"design_settings":{}},"net_settings":{"classes":[{"name":"Default","clearance":b.rules.clearance,"track_width":b.rules.track_width,"via_diameter":b.rules.via_diameter,"via_drill":b.rules.via_drill}],"meta":{"version":3}}}))?.as_bytes(),false)?;
                b.save(&path)?;
                e.board = Some(b);
                e.active_kind = "pcb".into();
                Ok(board(e)?.inspect(&json!({})))
            })();
            if result.is_err() {
                for path in std::fs::read_dir(&folder)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                {
                    if path.is_file() {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
            if let Ok(v) = &result {
                let _ = crate::catalog::record(
                    "create_project",
                    None,
                    v["file"].as_str().map(Path::new),
                );
            }
            result
        }
        "pcb_new" => {
            ensure!(
                !e.board.as_ref().is_some_and(|b| b.dirty),
                "Save/discard the current PCB before creating another"
            );
            let mut b = Board::new(
                p["width_mm"].as_f64().unwrap_or(80.),
                p["height_mm"].as_f64().unwrap_or(60.),
            )?;
            if let Some(path) = p["path"].as_str() {
                let path = e.resolve(path);
                ensure!(!path.exists(), "PCB destination already exists");
                b.path = Some(path);
            }
            e.board = Some(b);
            e.active_kind = "pcb".into();
            Ok(board(e)?.inspect(&json!({})))
        }
        "pcb_open" => {
            ensure!(
                !e.board.as_ref().is_some_and(|b| b.dirty) || p["discard_changes"] == true,
                "PCB has unsaved changes; save before switching"
            );
            let b = Board::open(&e.resolve(string(p, "path")?))?;
            e.board = Some(b);
            e.active_kind = "pcb".into();
            Ok(board(e)?.inspect(p))
        }
        "pcb_close" => {
            ensure!(
                !board(e)?.dirty || p["discard_changes"] == true,
                "PCB has unsaved changes; save first"
            );
            e.board = None;
            e.active_kind = "schematic".into();
            Ok(json!({"closed":true}))
        }
        "pcb_save" => {
            let path = p["path"]
                .as_str()
                .map(|p| e.resolve(p))
                .or_else(|| e.board.as_ref()?.path.clone())
                .context("PCB save needs path")?;
            let mut candidate = board(e)?.snapshot();
            if let Some(old) = candidate
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .map(Path::to_owned)
            {
                if Some(old.as_path()) != path.parent() {
                    for fp in candidate
                        .root
                        .items_mut()
                        .iter_mut()
                        .filter(|n| n.name() == "footprint")
                    {
                        for model in fp.items_mut().iter_mut().filter(|n| n.name() == "model") {
                            if let Some(resolved) = pcb_backend::resolve_model(model.value(1), &old)
                            {
                                model.items_mut()[1] = Node::string(resolved.to_string_lossy());
                            }
                        }
                    }
                }
            }
            candidate.save(&path)?;
            e.board = Some(candidate);
            let _ = crate::catalog::record("save", None, Some(&path));
            Ok(board(e)?.inspect(&json!({"include_pads":false})))
        }
        "pcb_inspect" => {
            if let Some(since) = p["since_revision"].as_str() {
                return changes_since(board(e)?, since);
            }
            Ok(board(e)?.inspect(p))
        }
        "pcb_trace" => {
            let b = board(e)?;
            let from = b.pad(&p["from"])?;
            let to = p.get("to").map(|p| b.pad(p)).transpose()?;
            let pads = b
                .pads()
                .into_iter()
                .filter(|pad| pcb_checks::connected(b, &from, pad))
                .collect::<Vec<_>>();
            Ok(
                json!({"revision":b.revision(),"from":from,"connected":to.as_ref().map(|to|pcb_checks::connected(b,&from,to)),"pads":pads}),
            )
        }
        "pcb_verify" => verify_with_source(board(e)?, e.doc.as_ref(), p),
        "pcb_3d" => {
            let revision = pcb_backend::mesh_revision(board(e)?);
            let path = p["path"].as_str().map(|p| e.resolve(p)).unwrap_or_else(|| {
                crate::catalog::directory()
                    .join("3d")
                    .join(format!("{}.glb", revision))
            });
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let cache_meta = path.with_extension("glb.jenga.json");
            let backend = if path.exists() {
                let prior: Value = std::fs::read(&cache_meta)
                    .ok()
                    .and_then(|s| serde_json::from_slice(&s).ok())
                    .unwrap_or(Value::Null);
                ensure!(
                    prior["mesh_revision"] == revision,
                    "Existing 3D output is stale or not owned by this board; choose a new path"
                );
                json!({"cached":true})
            } else {
                let result = pcb_backend::mesh(board(e)?, &path)?;
                atomic_write(
                    &cache_meta,
                    &serde_json::to_vec(&json!({"mesh_revision":revision}))?,
                    false,
                )?;
                result
            };
            let mesh = jenga_core::mesh::Mesh::read(&path)?;
            Ok(
                json!({"path":path,"revision":board(e)?.revision(),"mesh":mesh.info(),"models":pcb_backend::model_findings(board(e)?),"backend":backend}),
            )
        }
        "pcb_export" => export(e, p),
        "pcb_commit" => commit(e, p),
        "pcb_undo" | "pcb_redo" => {
            let b = e.board.as_mut().context("No PCB")?;
            let root = if method == "pcb_undo" {
                b.undo.pop()
            } else {
                b.redo.pop()
            }
            .context("No PCB history in this session")?;
            if method == "pcb_undo" {
                b.redo.push(b.state());
            } else {
                b.undo.push(b.state());
            }
            b.restore(root);
            Ok(b.inspect(&json!({"include_pads":false})))
        }
        "pcb_history" => {
            let b = board(e)?;
            let path = history_path(b);
            let mut entries = vec![];
            if let Some(path) = path {
                for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
                    if let Ok(mut v) = serde_json::from_str::<Value>(line) {
                        v.as_object_mut().unwrap().remove("before");
                        v.as_object_mut().unwrap().remove("after");
                        entries.push(v);
                    }
                }
            }
            let limit = p["limit"].as_u64().unwrap_or(30).min(1000) as usize;
            let start = entries.len().saturating_sub(limit);
            Ok(json!({"entries":&entries[start..]}))
        }
        "pcb_copy_project" | "pcb_rename_project" | "pcb_archive_project" => {
            project_operation(e, method, p)
        }
        "pcb_assign_footprints" => {
            let doc = e
                .doc
                .as_ref()
                .context("Open a schematic to assign footprints")?;
            let mut candidate = doc.clone();
            let map = p["footprints"]
                .as_object()
                .context("footprints must map component references to Library:Footprint IDs")?;
            let libs = jenga_libraries::Libraries::discover(&[], Some(&footprint_tools::folder(e)));
            for (reference, id) in map {
                let id = id.as_str().context("Footprint IDs must be strings")?;
                let def = libs
                    .footprint(id)
                    .with_context(|| format!("Footprint for {reference}: {id}"))?;
                let indices: Vec<_> = candidate
                    .root
                    .items()
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.name() == "symbol" && property(n, "Reference") == reference)
                    .map(|(i, _)| i)
                    .collect();
                ensure!(!indices.is_empty(), "Reference {reference} not found");
                let numbers: HashSet<_> = def
                    .children("pad")
                    .filter(|p| p.value(2) != "np_thru_hole")
                    .map(|p| p.value(1).to_owned())
                    .collect();
                let scene = Scene::from_document(&candidate);
                for pin in scene
                    .pins
                    .iter()
                    .filter(|pin| indices.contains(&pin.object))
                {
                    let mapped = p["pad_maps"][reference][&pin.number]
                        .as_str()
                        .unwrap_or(&pin.number);
                    ensure!(
                        numbers.contains(mapped),
                        "{} pin {} has no corresponding pad {} in {}",
                        reference,
                        pin.number,
                        mapped,
                        id
                    );
                }
                for i in indices {
                    candidate.set_property(i, "Footprint", id);
                }
            }
            if let Some(rev) = p["expected_revision"].as_str() {
                ensure!(
                    rev == crate::journal::revision(doc),
                    "Schematic revision conflict"
                );
            }
            if p["dry_run"] == true {
                return Ok(
                    json!({"applied":false,"assignments":map,"revision":crate::journal::revision(doc)}),
                );
            }
            if e.record_history {
                crate::journal::record(
                    doc,
                    &candidate,
                    p["actor"].as_str().unwrap_or("external"),
                    "assign_footprints",
                )?;
            }
            e.undo.push(doc.clone());
            e.doc = Some(candidate);
            Ok(
                json!({"assigned":map.len(),"revision":crate::journal::revision(e.doc.as_ref().unwrap()),"dirty":true}),
            )
        }
        "pcb_edit"
        | "pcb_preview"
        | "pcb_connect"
        | "pcb_sync"
        | "pcb_autoplace"
        | "pcb_autoroute"
        | "build_pcb_from_schematic"
        | "pcb_fill_zones" => transaction(e, method, p),
        _ => bail!("Unknown PCB operation {method}"),
    }
}

pub fn verify(b: &Board, p: &Value) -> Result<Value> {
    let mut result = pcb_checks::verify(b, p)?;
    result["models"] = pcb_backend::model_findings(b);
    if p["external"] == true {
        match pcb_backend::drc(b, false) {
            Ok((drc, _)) => {
                let errors = drc["violations"]
                    .as_array()
                    .map(|v| v.iter().filter(|v| v["severity"] == "error").count())
                    .unwrap_or(0);
                let unconnected = drc["unconnected_items"]
                    .as_array()
                    .map(Vec::len)
                    .unwrap_or(0);
                result["external_kicad_checked"] = json!(true);
                result["kicad_drc"] = drc;
                if errors > 0 || unconnected > 0 {
                    result["passed"] = json!(false);
                }
            }
            Err(err) => {
                result["external_kicad_checked"] = json!(false);
                result["backend_error"] = json!(format!("{err:#}"));
                result["passed"] = json!(false);
            }
        }
    }
    Ok(result)
}
fn verify_with_source(b: &Board, doc: Option<&Document>, p: &Value) -> Result<Value> {
    let mut v = verify(b, p)?;
    if p["include_schematic"] != false {
        if let Some(link) = &b.schematic {
            let disk = if !doc.is_some_and(|d| d.path.as_ref() == Some(link)) {
                Document::open(link).ok()
            } else {
                None
            };
            if let Some(doc) = doc
                .filter(|d| d.path.as_ref() == Some(link))
                .or(disk.as_ref())
            {
                let report = crate::workflows::verify(doc, &json!({}))?;
                if report["passed"] == false {
                    v["passed"] = json!(false);
                }
                v["schematic_verification"] = report;
            } else {
                v["schematic_verification"] =
                    json!({"passed":false,"error":"Linked schematic is unavailable","file":link});
                v["passed"] = json!(false);
            }
        }
    }
    Ok(v)
}
pub fn transaction(e: &mut Engine, method: &str, p: &Value) -> Result<Value> {
    let before = e.board.clone();
    if let Some(expected) = p["expected_revision"].as_str() {
        ensure!(
            before.as_ref().is_some_and(|b| b.revision() == expected),
            "PCB revision conflict; inspect again"
        );
    }
    if let Some(b) = &before {
        ensure!(
            !b.disk_changed()?,
            "PCB changed externally; reopen before editing"
        );
    }
    let mut candidate = if let Some(b) = &before {
        b.clone()
    } else {
        ensure!(
            method == "build_pcb_from_schematic" || method == "pcb_sync",
            "Open/create a PCB first"
        );
        let mut b = Board::new(
            p["width_mm"].as_f64().unwrap_or(80.),
            p["height_mm"].as_f64().unwrap_or(60.),
        )?;
        if let Some(path) = p["path"].as_str() {
            let path = e.resolve(path);
            ensure!(!path.exists(), "PCB destination exists");
            b.path = Some(path);
        }
        b
    };
    let folder = footprint_tools::folder(e);
    let libs = jenga_libraries::Libraries::discover(&[], Some(&folder));
    let mut results = vec![];
    let mut failures = vec![];
    if let Some(rules) = p.get("rules") {
        candidate.rules = serde_json::from_value(rules.clone())?;
        candidate.rules.validate()?;
    }
    let dry = p["dry_run"] == true
        || method == "pcb_preview"
        || (method == "build_pcb_from_schematic" && p["dry_run"] != false);
    if matches!(method, "pcb_sync" | "build_pcb_from_schematic") {
        let doc = e
            .doc
            .as_ref()
            .context("Open the intended schematic before synchronizing a PCB")?;
        let summary = sync(&mut candidate, doc, p, &libs)?;
        let added = summary["added_uuids"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        results.push(summary);
        if (method == "build_pcb_from_schematic" && p["autoplace"] != false)
            || p["autoplace"] == true
        {
            results.push(autoplace(
                &mut candidate,
                &json!({"uuids":added,"groups":p["groups"]}),
            )?);
        }
    }
    if method == "pcb_autoplace" {
        results.push(autoplace(&mut candidate, p)?);
    }
    let actions = p["actions"].as_array().cloned().unwrap_or_default();
    ensure!(actions.len() <= 10000, "Too many PCB actions");
    for (index, a) in actions.iter().enumerate() {
        let prior = candidate.clone();
        match action(&mut candidate, &libs, a) {
            Ok(mut result) => {
                result["action_index"] = json!(index);
                results.push(result);
            }
            Err(err) => {
                candidate = prior;
                if !dry {
                    return Err(err).with_context(|| {
                        format!("PCB action {index} {} failed; no edits committed", a["op"])
                    });
                }
                failures.push(json!({"index":index,"action":a,"error":format!("{err:#}")}));
            }
        }
    }
    if method == "pcb_connect" {
        for (index, pair) in p["connections"]
            .as_array()
            .context("pcb_connect requires connections")?
            .iter()
            .enumerate()
        {
            let prior = candidate.clone();
            let mut options = pair.clone();
            if p.get("reserved_corridors").is_some() {
                options["reserved_corridors"] = p["reserved_corridors"].clone();
            }
            match pcb_routing::connect(&mut candidate, &pair["from"], &pair["to"], &options) {
                Ok(v) => results.push(v),
                Err(err) => {
                    candidate = prior;
                    if !dry {
                        return Err(err).with_context(|| {
                            format!(
                                "PCB connection {index}: {} → {}; no edits committed",
                                pair["from"], pair["to"]
                            )
                        });
                    }
                    failures.push(json!({"index":index,"from":pair["from"],"to":pair["to"],"error":format!("{err:#}")}));
                }
            }
        }
    }
    if method == "pcb_autoroute"
        || (method == "build_pcb_from_schematic" && p["route"] != false)
        || (method == "pcb_sync" && p["route"] == true)
    {
        let pairs = pcb_checks::ratsnest(&candidate);
        ensure!(
            pairs.len() <= 2000,
            "Automatic routing is limited to 2000 initial connections"
        );
        for (a, b) in pairs {
            let from = json!({"reference":a.reference,"number":a.number});
            let to = json!({"reference":b.reference,"number":b.number});
            let prior = candidate.clone();
            let mut route_options = p.get("route_options").cloned().unwrap_or_else(|| p.clone());
            if method == "build_pcb_from_schematic" {
                route_options.as_object_mut().unwrap().remove("width_mm");
                route_options.as_object_mut().unwrap().remove("height_mm");
                if let Some(width) = p.get("track_width_mm") {
                    route_options["width_mm"] = width.clone();
                }
            }
            match pcb_routing::connect(&mut candidate, &from, &to, &route_options) {
                Ok(v) => results.push(v),
                Err(err) => {
                    candidate = prior;
                    failures.push(json!({"from":from,"to":to,"error":format!("{err:#}")}));
                }
            }
        }
    }
    if method == "pcb_fill_zones" {
        let (result, root) = pcb_backend::drc(&candidate, true)?;
        candidate.root = root.context("Zone refill produced no board")?;
        candidate.dirty = true;
        results.push(json!({"zone_fill":result}));
    }
    let verification = verify_with_source(&candidate, e.doc.as_ref(), p)?;
    if p["external"] == true && verification["passed"] != true {
        failures.push(json!({"kind":"verification_failed","message":"Native verification or electrical intent failed; inspect findings and unrouted pads before committing","findings":verification["findings"],"unrouted":verification["unrouted"]}));
    }
    let baseline = before
        .as_ref()
        .map(|b| pcb_checks::verify(b, &json!({})))
        .transpose()?
        .unwrap_or(json!({"findings":[]}));
    let old: HashSet<_> = baseline["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["kind"] == "short")
        .map(|f| f["objects"].to_string())
        .collect();
    let new_shorts: Vec<_> = verification["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["kind"] == "short" && !old.contains(&f["objects"].to_string()))
        .cloned()
        .collect();
    if !new_shorts.is_empty() && p["allow_shorts"] != true {
        failures.push(json!({"kind":"unintended_short","findings":new_shorts}));
    }
    if verification["connections"]["passed"] != true {
        failures.push(
            json!({"kind":"connection_intent","findings":verification["connections"]["findings"]}),
        );
    }
    let can_commit = failures.is_empty()
        || (p["allow_partial"] == true
            && !failures.iter().any(|f| {
                matches!(
                    f["kind"].as_str(),
                    Some("unintended_short" | "connection_intent" | "verification_failed")
                )
            }));
    if dry {
        return preview(
            e,
            before.as_ref(),
            &candidate,
            json!({"results":results,"failures":failures,"can_commit":can_commit,"verification":verification}),
        );
    }
    ensure!(
        can_commit,
        "PCB batch failed; no edits committed: {}",
        json!(failures)
    );
    apply(e, before, candidate, p, method)?;
    Ok(
        json!({"revision":board(e)?.revision(),"dirty":true,"results":results,"failures":failures,"verification":verification,"complete":verification["passed"]}),
    )
}
fn export_pages(e: &Engine, p: &Value) -> Result<Value> {
    let folder = p["project_folder"]
        .as_str()
        .map(|v| e.resolve(v))
        .unwrap_or_else(|| footprint_tools::folder(e));
    let mut project = crate::project_workspace::discover(&folder)?;
    project.boards.sort();
    let selected = p["pages"].as_array();
    let boards: Vec<_> = project
        .boards
        .into_iter()
        .filter(|path| {
            selected.is_none_or(|names| {
                names.iter().any(|n| {
                    n.as_str().is_some_and(|n| {
                        n == path.to_string_lossy()
                            || Some(n) == path.file_stem().and_then(|p| p.to_str())
                    })
                })
            })
        })
        .collect();
    ensure!(!boards.is_empty(), "No PCB pages match the selection");
    if let Some(names) = selected {
        ensure!(
            names.len() == boards.len(),
            "PCB page selection contains missing or duplicate names"
        );
    }
    let output = e.resolve(string(p, "path")?);
    ensure!(!output.exists(), "PCB page export destination exists");
    let format = p["format"].as_str().unwrap_or("pdf");
    let mut files = vec![];
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if format == "pdf" {
        let temp = tempfile::tempdir()?;
        let mut pages = vec![];
        for (i, path) in boards.iter().enumerate() {
            let board = Board::open(path)?;
            let image = temp.path().join(format!("{i}.png"));
            let scene = board.scene(
                Some(&[
                    "F.Cu".into(),
                    "B.Cu".into(),
                    "F.SilkS".into(),
                    "Edge.Cuts".into(),
                ]),
                false,
            );
            crate::render::preview(
                &scene,
                &image,
                p["width"].as_u64().unwrap_or(2000).min(4096) as u32,
            )?;
            pages.push((
                format!(
                    "PCB - {}",
                    path.file_stem().unwrap_or_default().to_string_lossy()
                ),
                image::open(image)?.to_rgb8(),
            ));
        }
        crate::assembly_view::pdf(&output, &pages)?;
        files.push(output.clone());
    } else {
        ensure!(
            matches!(format, "png" | "svg" | "native"),
            "PCB page format must be pdf, png, svg or native"
        );
        std::fs::create_dir(&output)?;
        for path in &boards {
            let board = Board::open(path)?;
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            let dest = output.join(format!(
                "{name}.{}",
                if format == "native" {
                    "kicad_pcb"
                } else {
                    format
                }
            ));
            if format == "native" {
                let temp = tempfile::tempdir()?;
                let prepared = pcb_backend::prepared(&board, temp.path())?;
                std::fs::copy(prepared, &dest)?;
            } else if format == "png" {
                crate::render::preview(
                    &board.scene(
                        Some(&[
                            "F.Cu".into(),
                            "B.Cu".into(),
                            "F.SilkS".into(),
                            "Edge.Cuts".into(),
                        ]),
                        false,
                    ),
                    &dest,
                    p["width"].as_u64().unwrap_or(2000).min(4096) as u32,
                )?;
            } else {
                pcb_backend::export(&board, &dest, "svg")?;
            }
            files.push(dest);
        }
    }
    Ok(
        json!({"path":output,"format":format,"page_count":boards.len(),"boards":boards,"files":files,"page_semantics":"each page is an independent physical PCB"}),
    )
}
fn action(b: &mut Board, libs: &jenga_libraries::Libraries, a: &Value) -> Result<Value> {
    let op = match string(a, "op")? {
        "move" => "move_footprint",
        "rotate" => "rotate_footprint",
        "flip" => "flip_footprint",
        op => op,
    };
    match op {
        "add_footprint" => {
            let id = string(a, "library_id")?;
            let def = libs.footprint(id)?;
            let uuid = b.add_footprint(
                id,
                &def,
                string(a, "reference")?,
                a["value"].as_str().unwrap_or(""),
                pcb::json_point(a, "at")?,
                a["rotation"].as_f64().unwrap_or(0.),
            )?;
            return Ok(json!({"uuid":uuid,"reference":a["reference"]}));
        }
        "net" => {
            let code = b.ensure_net(string(a, "name")?);
            return Ok(json!({"code":code,"name":a["name"]}));
        }
        "pad_net" => {
            let i = b.target(a)?;
            let name = string(a, "net")?;
            let code = b.ensure_net(name);
            b.set_pad_net(i, string(a, "number")?, code)?;
        }
        "track" => {
            let points = a["points"].as_array().context("Track requires points")?;
            ensure!(
                points.len() >= 2 && points.len() <= 10000,
                "Track needs 2–10000 points"
            );
            let net = if let Some(name) = a["net"].as_str() {
                b.ensure_net(name)
            } else {
                a["net"].as_i64().context("Track requires net name/code")?
            };
            let width = a["width_mm"].as_f64().unwrap_or(b.rules.track_width);
            let layer = a["layer"].as_str().unwrap_or("F.Cu");
            let points = points
                .iter()
                .map(|p| pcb::json_point(&json!({"p":p}), "p"))
                .collect::<Result<Vec<_>>>()?;
            for w in points.windows(2) {
                ensure!(
                    pcb_checks::safe_track(b, w[0], w[1], width, layer, net)
                        || a["allow_rule_violations"] == true,
                    "Track crosses a pad/track/keepout or violates an edge/clearance rule at {:?} → {:?}",
                    w[0],
                    w[1]
                );
                b.segment(w[0], w[1], width, layer, net)?;
            }
        }
        "connect_pads" => return pcb_routing::connect(b, &a["from"], &a["to"], a),
        "via" => {
            let at = pcb::json_point(a, "at")?;
            let code = if let Some(name) = a["net"].as_str() {
                b.ensure_net(name)
            } else {
                a["net"].as_i64().context("Via requires net")?
            };
            b.via(at, code)?;
        }
        "outline" => {
            let points = a["points"]
                .as_array()
                .context("Outline needs points")?
                .iter()
                .map(|p| pcb::json_point(&json!({"p":p}), "p"))
                .collect::<Result<Vec<_>>>()?;
            b.outline(&points)?;
        }
        "rules" => {
            b.rules = serde_json::from_value(a["rules"].clone())?;
            b.rules.validate()?;
        }
        "stackup" => {
            let count = a["copper_layers"].as_u64().unwrap_or(2);
            ensure!(
                (2..=32).contains(&count) && count % 2 == 0,
                "Copper layer count must be even, 2–32"
            );
            ensure!(
                b.root
                    .children("segment")
                    .all(|s| ["F.Cu", "B.Cu"].contains(&pcb::layer(s))),
                "Inner layer traces must be removed/remapped before changing stackup"
            );
            let modern_layers = b.root.child("layers").is_some_and(|n| {
                n.items()
                    .iter()
                    .any(|n| n.value(1) == "B.Cu" && n.number(0) == 2.)
            });
            let layers = b.root.child_mut("layers").context("Missing layers")?;
            layers.items_mut().retain(|l| !l.value(1).ends_with(".Cu"));
            let mut copper = vec![Node::List(vec![
                Node::atom(0),
                Node::string("F.Cu"),
                Node::atom("signal"),
            ])];
            for i in 1..count - 1 {
                copper.push(Node::List(vec![
                    Node::atom(if modern_layers { (i + 1) * 2 } else { i }),
                    Node::string(format!("In{i}.Cu")),
                    Node::atom("signal"),
                ]));
            }
            copper.push(Node::List(vec![
                Node::atom(if modern_layers { 2 } else { 31 }),
                Node::string("B.Cu"),
                Node::atom("signal"),
            ]));
            for (i, l) in copper.into_iter().enumerate() {
                layers.items_mut().insert(i + 1, l);
            }
            let thickness = a["thickness_mm"].as_f64().unwrap_or(1.6);
            ensure!(
                thickness > 0. && thickness < 100.,
                "Invalid board thickness"
            );
            b.root
                .child_mut("general")
                .context("Missing general board properties")?
                .set_child(pcb::node("thickness", vec![Node::atom(thickness)]));
        }
        "move_footprint" | "rotate_footprint" | "flip_footprint" => {
            let i = b.target(a)?;
            ensure!(
                b.root.items()[i].name() == "footprint",
                "Target must be a footprint"
            );
            let before = b
                .pads()
                .into_iter()
                .filter(|p| p.object == i)
                .collect::<Vec<_>>();
            let old = b.root.items()[i].clone();
            let at = if op == "move_footprint" {
                pcb::json_point(a, "at")?
            } else {
                pcb::point(&old, "at")
            };
            let rotation = if op == "rotate_footprint" {
                a["rotation"]
                    .as_f64()
                    .unwrap_or(pcb::angle(&old) + a["angle"].as_f64().unwrap_or(90.))
            } else {
                a["rotation"].as_f64().unwrap_or(pcb::angle(&old))
            };
            b.place_footprint(i, at, rotation);
            if op == "flip_footprint" {
                flip_footprint(&mut b.root.items_mut()[i]);
            }
            preserve_routes(b, &before, a)?;
        }
        "replace_footprint" | "refresh_footprint" => {
            let i = b.target(a)?;
            let old = b.root.items()[i].clone();
            ensure!(old.name() == "footprint", "Target must be a footprint");
            let id = if op == "replace_footprint" {
                string(a, "library_id")?
            } else {
                old.value(1)
            };
            let def = libs.footprint(id)?;
            let prior = b
                .pads()
                .into_iter()
                .filter(|p| p.object == i)
                .collect::<Vec<_>>();
            let reference = pcb::field(&old, "Reference").to_owned();
            let value = pcb::field(&old, "Value").to_owned();
            let mut temp = Board::new(10., 10.)?;
            temp.add_footprint(
                id,
                &def,
                &reference,
                &value,
                pcb::point(&old, "at"),
                pcb::angle(&old),
            )?;
            let mut new = temp.root.children("footprint").next().unwrap().clone();
            if pcb::layer(&old) == "B.Cu" {
                flip_footprint(&mut new);
            }
            new.set_child(pcb::node(
                if old.child("tstamp").is_some() {
                    "tstamp"
                } else {
                    "uuid"
                },
                vec![Node::string(pcb::id(&old))],
            ));
            if let Some(path) = old.child("path") {
                new.set_child(path.clone());
            }
            let mut used = HashSet::new();
            let mut mapped = vec![];
            for mut pad in prior {
                let number = a["pad_map"][&pad.number].as_str().unwrap_or(&pad.number);
                ensure!(
                    used.insert(number.to_owned()),
                    "Multiple old pads map to {number}"
                );
                let matches: Vec<_> = new
                    .children("pad")
                    .filter(|p| p.value(1) == number)
                    .collect();
                ensure!(
                    matches.len() == 1,
                    "{} pad {} cannot map unambiguously to {} pad {}; provide pad_map",
                    reference,
                    pad.number,
                    id,
                    number
                );
                let name = b.nets().get(&pad.net).cloned().unwrap_or_default();
                for p in new
                    .items_mut()
                    .iter_mut()
                    .filter(|p| p.name() == "pad" && p.value(1) == number)
                {
                    if pad.net > 0 {
                        p.set_child(if b.named_nets() {
                            b.net_node(pad.net)
                        } else {
                            pcb::node("net", vec![Node::atom(pad.net), Node::string(&name)])
                        });
                    }
                }
                pad.number = number.into();
                mapped.push(pad);
            }
            b.root.items_mut()[i] = new;
            preserve_routes(b, &mapped, a)?;
            return Ok(
                json!({"reference":reference,"library_id":id,"pad_map":a["pad_map"],"wiring_preserved":true}),
            );
        }
        "set_property" => {
            let i = b.target(a)?;
            ensure!(
                b.root.items()[i].name() == "footprint",
                "Properties apply to footprints"
            );
            pcb::set_field(
                &mut b.root.items_mut()[i],
                string(a, "key")?,
                string(a, "value")?,
            );
        }
        "model" => {
            let i = b.target(a)?;
            ensure!(
                b.root.items()[i].name() == "footprint",
                "Model belongs to a footprint"
            );
            let path = string(a, "path")?;
            if a["replace"] == true {
                b.root.items_mut()[i]
                    .items_mut()
                    .retain(|n| n.name() != "model");
            }
            let mut model = pcb::node("model", vec![Node::string(path)]);
            for (key, default) in [
                ("offset", [0., 0., 0.]),
                ("scale", [1., 1., 1.]),
                ("rotate", [0., 0., 0.]),
            ] {
                let values = a[key]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|v| v.as_f64().unwrap_or(f64::NAN))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or(default.to_vec());
                ensure!(
                    values.len() == 3 && values.iter().all(|n| n.is_finite()),
                    "Model {key} needs three finite coordinates"
                );
                if key == "scale" {
                    ensure!(
                        values.iter().all(|v| *v > 0.),
                        "Model scale must be positive"
                    );
                }
                model.set_child(pcb::node(
                    key,
                    vec![pcb::node("xyz", values.iter().map(Node::atom).collect())],
                ));
            }
            b.root.items_mut()[i].items_mut().push(model);
        }
        "zone" | "keepout" => {
            let points = a["points"]
                .as_array()
                .context("Zone needs polygon points")?;
            ensure!(points.len() >= 3, "Zone requires at least 3 points");
            let layer = a["layer"].as_str().unwrap_or("F.Cu");
            let net = if op == "zone" {
                b.ensure_net(string(a, "net")?)
            } else {
                0
            };
            let mut zone = pcb::node(
                "zone",
                vec![
                    pcb::node("net", vec![Node::atom(net)]),
                    pcb::node(
                        "net_name",
                        vec![Node::string(
                            b.nets().get(&net).cloned().unwrap_or_default(),
                        )],
                    ),
                    pcb::node("layer", vec![Node::string(layer)]),
                    pcb::uid(),
                    pcb::node("hatch", vec![Node::atom("edge"), Node::atom(0.5)]),
                    pcb::node(
                        "connect_pads",
                        vec![pcb::node("clearance", vec![Node::atom(b.rules.clearance)])],
                    ),
                    pcb::node("min_thickness", vec![Node::atom(0.25)]),
                    pcb::node(
                        "fill",
                        vec![
                            Node::atom("yes"),
                            pcb::node("thermal_gap", vec![Node::atom(0.3)]),
                            pcb::node("thermal_bridge_width", vec![Node::atom(0.3)]),
                        ],
                    ),
                    pcb::node(
                        "polygon",
                        vec![pcb::node(
                            "pts",
                            points
                                .iter()
                                .map(|p| {
                                    pcb::json_point(&json!({"p":p}), "p").map(|p| pcb::xy("xy", p))
                                })
                                .collect::<Result<_>>()?,
                        )],
                    ),
                ],
            );
            zone.set_child(b.net_node(net));
            if b.named_nets() {
                zone.items_mut().retain(|n| n.name() != "net_name");
            }
            if op == "keepout" {
                zone.set_child(pcb::node(
                    "keepout",
                    vec![
                        pcb::node("tracks", vec![Node::atom("not_allowed")]),
                        pcb::node("vias", vec![Node::atom("not_allowed")]),
                        pcb::node("pads", vec![Node::atom("not_allowed")]),
                        pcb::node("copperpour", vec![Node::atom("not_allowed")]),
                        pcb::node("footprints", vec![Node::atom("not_allowed")]),
                    ],
                ));
            }
            b.root.items_mut().push(zone);
        }
        "text" => {
            let at = pcb::json_point(a, "at")?;
            let size = a["size_mm"].as_f64().unwrap_or(1.);
            ensure!(size > 0. && size < 100., "Invalid PCB text size");
            b.root.items_mut().push(pcb::node(
                "gr_text",
                vec![
                    Node::string(string(a, "text")?),
                    pcb::node(
                        "at",
                        vec![
                            Node::atom(at.0),
                            Node::atom(at.1),
                            Node::atom(a["rotation"].as_f64().unwrap_or(0.)),
                        ],
                    ),
                    pcb::node(
                        "layer",
                        vec![Node::string(a["layer"].as_str().unwrap_or("F.SilkS"))],
                    ),
                    pcb::node(
                        "effects",
                        vec![pcb::node(
                            "font",
                            vec![
                                pcb::xy("size", (size, size)),
                                pcb::node("thickness", vec![Node::atom(0.15)]),
                            ],
                        )],
                    ),
                    pcb::uid(),
                ],
            ));
        }
        "delete" => {
            let i = b.target(a)?;
            ensure!(
                !matches!(
                    b.root.items()[i].name(),
                    "layers" | "setup" | "general" | "net"
                ),
                "Cannot delete structural board metadata"
            );
            b.root.items_mut().remove(i);
        }
        "cleanup" => {
            let mut seen = HashSet::new();
            let mut removed = 0;
            b.root.items_mut().retain(|n| {
                if n.name() == "segment" {
                    let a = pcb::point(n, "start");
                    let c = pcb::point(n, "end");
                    let mut ends = [format!("{a:?}"), format!("{c:?}")];
                    ends.sort();
                    let key = format!(
                        "{}:{}:{}:{:?}",
                        pcb::net_code(n),
                        pcb::layer(n),
                        n.child("width").map(|n| n.number(1)).unwrap_or(0.),
                        ends
                    );
                    if a == c || !seen.insert(key) {
                        removed += 1;
                        return false;
                    }
                }
                true
            });
            return Ok(json!({"removed":removed}));
        }
        _ => bail!("Unknown PCB edit {op}"),
    }
    b.dirty = true;
    Ok(json!({"op":op}))
}
fn preserve_routes(b: &mut Board, old: &[pcb::Pad], options: &Value) -> Result<()> {
    if options["preserve_routes"] == false {
        return Ok(());
    }
    let current = b.pads();
    for p in old {
        if p.net <= 0 {
            continue;
        }
        let Some(new) = current
            .iter()
            .find(|n| n.reference == p.reference && n.number == p.number)
        else {
            bail!("Missing mapped pad {}.{}", p.reference, p.number)
        };
        if new.at == p.at {
            continue;
        }
        let connected = pcb_checks::copper(b).iter().any(|n| {
            n.net == p.net
                && n.kind != "pad"
                && n.geometry.distance(&pcb_checks::pad_geometry(p)) <= 1e-7
        });
        if connected {
            let (path, layers) =
                pcb_routing::route_points(b, new.at, p.at, p.net, &new.layers, &p.layers, options)
                    .with_context(|| {
                        format!("Cannot preserve tracks on {}.{}", p.reference, p.number)
                    })?;
            for i in 1..path.len() {
                if layers[i] != layers[i - 1] {
                    b.via(path[i], p.net)?;
                } else if path[i] != path[i - 1] {
                    b.segment(path[i - 1], path[i], b.rules.track_width, &layers[i], p.net)?;
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
struct Part {
    reference: String,
    value: String,
    footprint: String,
    uuid: String,
    pins: Vec<String>,
}
fn design(doc: &Document) -> Result<(Vec<Part>, Vec<(String, Vec<(String, String)>)>)> {
    if doc.root.children("sheet").next().is_some() {
        ensure!(
            !doc.dirty,
            "Save hierarchical schematics before project-wide PCB synchronization"
        );
        let path = doc
            .path
            .as_ref()
            .context("Save the hierarchical root first")?;
        let temp = tempfile::tempdir()?;
        let out = temp.path().join("netlist.net");
        let args = vec![
            "sch".into(),
            "export".into(),
            "netlist".into(),
            "--format".into(),
            "kicadsexpr".into(),
            "--output".into(),
            out.to_string_lossy().into(),
            path.to_string_lossy().into(),
        ];
        crate::pcb_backend::run(&args, Duration::from_secs(60))?;
        let root = parse(&std::fs::read_to_string(out)?)?;
        let mut parts = vec![];
        for comp in root
            .child("components")
            .context("Native netlist has no components")?
            .children("comp")
        {
            let reference = comp.child("ref").map(|n| n.value(1)).unwrap_or("");
            if reference.starts_with('#') {
                continue;
            }
            parts.push(Part {
                reference: reference.into(),
                value: comp.child("value").map(|n| n.value(1)).unwrap_or("").into(),
                footprint: comp
                    .child("footprint")
                    .map(|n| n.value(1))
                    .unwrap_or("")
                    .into(),
                uuid: comp
                    .child("tstamps")
                    .map(|n| n.value(1))
                    .unwrap_or("")
                    .into(),
                pins: vec![],
            });
        }
        let nets = root
            .child("nets")
            .context("Native netlist has no nets")?
            .children("net")
            .map(|n| {
                (
                    n.child("name").map(|n| n.value(1)).unwrap_or("").to_owned(),
                    n.children("node")
                        .map(|p| {
                            (
                                p.child("ref").map(|n| n.value(1)).unwrap_or("").to_owned(),
                                p.child("pin").map(|n| n.value(1)).unwrap_or("").to_owned(),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        for part in &mut parts {
            part.pins = nets
                .iter()
                .flat_map(|(_, pins)| pins)
                .filter(|(reference, _)| reference == &part.reference)
                .map(|(_, pin)| pin.clone())
                .collect();
            part.pins.sort();
            part.pins.dedup();
        }
        return Ok((parts, nets));
    }
    let scene = Scene::from_document(doc);
    ensure!(
        scene.missing.is_empty(),
        "Schematic has unresolved symbol definitions: {:?}",
        scene.missing
    );
    let mut parts: BTreeMap<String, Part> = BTreeMap::new();
    for (i, sym) in doc
        .root
        .items()
        .iter()
        .enumerate()
        .filter(|(_, n)| n.name() == "symbol")
    {
        let reference = property(sym, "Reference");
        if reference.starts_with('#')
            || sym.child("on_board").is_some_and(|n| n.value(1) == "no")
            || doc
                .instance_definition(sym)
                .is_some_and(|n| n.child("power").is_some())
        {
            continue;
        }
        let part = parts.entry(reference.into()).or_insert_with(|| Part {
            reference: reference.into(),
            value: property(sym, "Value").into(),
            footprint: property(sym, "Footprint").into(),
            uuid: sym.child("uuid").map(|n| n.value(1)).unwrap_or("").into(),
            pins: vec![],
        });
        if part.footprint.is_empty() {
            part.footprint = property(sym, "Footprint").into();
        }
        part.pins.extend(
            scene
                .pins
                .iter()
                .filter(|p| p.object == i)
                .map(|p| p.number.clone()),
        );
    }
    for p in parts.values_mut() {
        p.pins.sort();
        p.pins.dedup();
    }
    let netlist = jenga_core::connectivity::inspect(doc);
    let nets = netlist["nets"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let pins = n["pins"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|p| {
                    let r = p["reference"].as_str()?;
                    parts
                        .contains_key(r)
                        .then(|| (r.to_owned(), p["number"].as_str().unwrap_or("").to_owned()))
                })
                .collect::<Vec<_>>();
            if pins.is_empty() {
                return None;
            }
            let name = n["names"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .next()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Net-{}-{}", pins[0].0, i + 1));
            Some((name, pins))
        })
        .collect();
    Ok((parts.into_values().collect(), nets))
}
pub fn preflight(doc: &Document, p: &Value, libs: &jenga_libraries::Libraries) -> Result<Value> {
    let (parts, _) = design(doc)?;
    let mut findings = vec![];
    for part in &parts {
        let id = p["footprints"][&part.reference]
            .as_str()
            .unwrap_or(&part.footprint);
        if id.is_empty() {
            findings.push(json!({"reference":part.reference,"value":part.value,"kind":"missing_footprint","message":"Choose a footprint matching the real component package"}));
            continue;
        }
        match libs.footprint(id){
            Err(err)=>findings.push(json!({"reference":part.reference,"value":part.value,"footprint":id,"kind":"footprint_not_found","message":err.to_string()})),
            Ok(def)=>{let pads:HashSet<_>=def.children("pad").filter(|p|p.value(2)!="np_thru_hole").map(|p|p.value(1)).collect();
                for pin in &part.pins {let mapped=p["pad_maps"][&part.reference][pin].as_str().unwrap_or(pin);
                    if !pads.contains(mapped){findings.push(json!({"reference":part.reference,"value":part.value,"footprint":id,"pin":pin,"pad":mapped,"kind":"pin_pad_mismatch","message":"Physical symbol pin has no matching footprint pad; choose the correct package or explicit pad map"}));}
                }
            }
        }
    }
    Ok(
        json!({"passed":findings.is_empty(),"part_count":parts.len(),"finding_count":findings.len(),"findings":findings,"revision":crate::journal::revision(doc),"applied":false}),
    )
}
fn sync(
    b: &mut Board,
    doc: &Document,
    p: &Value,
    libs: &jenga_libraries::Libraries,
) -> Result<Value> {
    let (parts, nets) = design(doc)?;
    let mut missing = vec![];
    let mut defs = BTreeMap::new();
    for part in &parts {
        let id = p["footprints"][&part.reference]
            .as_str()
            .unwrap_or(&part.footprint);
        if id.is_empty() {
            missing.push(format!("{}: missing footprint assignment", part.reference));
            continue;
        }
        match libs.footprint(id) {
            Ok(def) => {
                let pads: HashSet<_> = def
                    .children("pad")
                    .filter(|p| p.value(2) != "np_thru_hole")
                    .map(|p| p.value(1))
                    .collect();
                for pin in &part.pins {
                    let mapped = p["pad_maps"][&part.reference][pin].as_str().unwrap_or(pin);
                    if !pads.contains(mapped) {
                        missing.push(format!(
                            "{} pin {} → pad {} missing in {}",
                            part.reference, pin, mapped, id
                        ));
                    }
                }
                defs.insert(part.reference.clone(), (id.to_owned(), def));
            }
            Err(err) => missing.push(format!("{}: {}: {err}", part.reference, id)),
        }
    }
    ensure!(
        missing.is_empty(),
        "PCB synchronization preflight failed: {}",
        format!(
            "\n{}\nAssign an existing or custom footprint to each listed symbol, then preview again. No PCB edits applied.",
            missing.join("\n")
        )
    );
    let mut added = vec![];
    let mut updated = vec![];
    for part in &parts {
        let (id, def) = &defs[&part.reference];
        let i = if let Some(i) =
            b.root.items().iter().position(|n| {
                n.name() == "footprint" && pcb::field(n, "Reference") == part.reference
            }) {
            if b.root.items()[i].value(1) != id {
                ensure!(
                    p["update_footprints"] == true,
                    "{} footprint changed from {} to {}; preview with update_footprints:true and pad_maps",
                    part.reference,
                    b.root.items()[i].value(1),
                    id
                );
                action(
                    b,
                    libs,
                    &json!({"op":"replace_footprint","reference":part.reference,"library_id":id,"pad_map":p["pad_maps"][&part.reference]}),
                )?;
            }
            updated.push(part.reference.clone());
            i
        } else {
            let uuid = b.add_footprint(id, def, &part.reference, &part.value, (20., 20.), 0.)?;
            added.push(uuid);
            b.root.items().len() - 1
        };
        pcb::set_field(&mut b.root.items_mut()[i], "Value", &part.value);
        if !part.uuid.is_empty() {
            b.root.items_mut()[i].set_child(pcb::node(
                "path",
                vec![Node::string(format!("/{}/{}", doc.root_uuid(), part.uuid))],
            ));
        }
    }
    let old_nets = b.pads();
    for (name, pins) in nets {
        let code = b.ensure_net(&name);
        for (reference, number) in pins {
            if let Some(i) =
                b.root.items().iter().position(|n| {
                    n.name() == "footprint" && pcb::field(n, "Reference") == reference
                })
            {
                let number = p["pad_maps"][&reference][&number]
                    .as_str()
                    .unwrap_or(&number);
                b.set_pad_net(i, number, code)?;
            }
        }
    }
    b.schematic = if p["link"] == false {
        None
    } else {
        doc.path.clone()
    };
    b.dirty = true;
    let changed_nets:Vec<_>=b.pads().iter().filter_map(|new|old_nets.iter().find(|old|old.uuid==new.uuid).filter(|old|old.net_name!=new.net_name).map(|old|json!({"reference":new.reference,"pad":new.number,"before":old.net_name,"after":new.net_name,"at_mm":new.at}))).collect();
    let refs: HashSet<_> = parts.iter().map(|p| p.reference.as_str()).collect();
    let obsolete: Vec<_> = b
        .root
        .children("footprint")
        .filter(|f| !refs.contains(pcb::field(f, "Reference")))
        .map(|f| pcb::field(f, "Reference").to_owned())
        .collect();
    Ok(
        json!({"added_uuids":added,"updated_references":updated,"changed_pad_nets":changed_nets,"other_board_footprints_retained":obsolete,"schematic":b.schematic,"source_revision":crate::journal::revision(doc),"analysis_scope":if doc.root.children("sheet").next().is_some(){"native_hierarchical_netlist"}else{"single_schematic"}}),
    )
}
fn autoplace(b: &mut Board, p: &Value) -> Result<Value> {
    let ids = p["uuids"].as_array();
    let indices: Vec<_> = b
        .root
        .items()
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            n.name() == "footprint" && ids.is_none_or(|ids| ids.contains(&json!(pcb::id(n))))
        })
        .map(|(i, _)| i)
        .collect();
    let selected: HashSet<_> = indices.iter().copied().collect();
    let old_pads = b.pads();
    for pad in old_pads.iter().filter(|p| selected.contains(&p.object)) {
        ensure!(
            !b.root
                .children("segment")
                .any(|n| pcb::net_code(n) == pad.net
                    && pcb_checks::point_segment(
                        pad.at,
                        pcb::point(n, "start"),
                        pcb::point(n, "end")
                    ) < 0.01),
            "Autoplace refuses wired {}.{}; move it explicitly with preserved routes",
            pad.reference,
            pad.number
        );
    }
    let (lo, hi) = b.bounds();
    let keepouts: Vec<_> = b
        .root
        .children("zone")
        .filter(|n| {
            n.child("keepout").is_some_and(|k| {
                k.child("footprints")
                    .is_some_and(|n| n.value(1) == "not_allowed")
            })
        })
        .filter_map(|n| n.child("polygon"))
        .map(|n| pcb_checks::Geometry::Polygon(pcb::pts(n)))
        .collect();
    let mut occupied = vec![];
    let scene = b.scene(
        Some(&[
            "F.CrtYd".into(),
            "B.CrtYd".into(),
            "F.SilkS".into(),
            "B.SilkS".into(),
            "*.Cu".into(),
        ]),
        false,
    );
    for (i, _n) in b
        .root
        .items()
        .iter()
        .enumerate()
        .filter(|(_, n)| n.name() == "footprint")
    {
        if !selected.contains(&i) {
            if let Some(bounds) = scene.object_bounds(i) {
                occupied.push(bounds);
            }
        }
    }
    let mut ordered = indices.clone();
    ordered.sort_by_key(|i| {
        let r = pcb::field(&b.root.items()[*i], "Reference");
        (
            if r.starts_with('U') {
                0
            } else if r.starts_with('J') || r.starts_with('P') {
                1
            } else {
                2
            },
            r.to_owned(),
        )
    });
    let mut placed: Vec<(usize, Point)> = vec![];
    for i in ordered {
        let def = &b.root.items()[i];
        let reference = pcb::field(def, "Reference").to_owned();
        let mut physical = def.clone();
        physical
            .items_mut()
            .retain(|n| !matches!(n.name(), "property" | "fp_text"));
        let raw = Scene::footprint_preview(&physical);
        let (a, c) = raw.content_bounds();
        let w = (c.0 - a.0).max(1.);
        let h = (c.1 - a.1).max(1.);
        let own_nets: HashSet<_> = old_pads
            .iter()
            .filter(|p| p.object == i && p.net > 0)
            .map(|p| p.net)
            .collect();
        let neighbors: Vec<_> = b
            .pads()
            .into_iter()
            .filter(|p| {
                p.object != i
                    && own_nets.contains(&p.net)
                    && (!selected.contains(&p.object) || placed.iter().any(|(j, _)| *j == p.object))
            })
            .map(|p| p.at)
            .collect();
        let target = if let Some(group) = p["groups"].as_array().into_iter().flatten().find(|g| {
            g["references"]
                .as_array()
                .is_some_and(|r| r.contains(&json!(reference)))
                && g["at"].is_array()
        }) {
            pcb::json_point(group, "at")?
        } else if !neighbors.is_empty() {
            (
                neighbors.iter().map(|p| p.0).sum::<f64>() / neighbors.len() as f64,
                neighbors.iter().map(|p| p.1).sum::<f64>() / neighbors.len() as f64,
            )
        } else if reference.starts_with('J') {
            (lo.0 + 5., (lo.1 + hi.1) / 2.)
        } else {
            ((lo.0 + hi.0) / 2., (lo.1 + hi.1) / 2.)
        };
        let mut candidates = vec![];
        let step = b.rules.grid.max(0.5);
        let nx = ((hi.0 - lo.0 - w - 6.) / step).floor() as i32;
        let ny = ((hi.1 - lo.1 - h - 6.) / step).floor() as i32;
        for x in 0..=nx {
            for y in 0..=ny {
                let min = (lo.0 + 3. + x as f64 * step, lo.1 + 3. + y as f64 * step);
                let max = (min.0 + w, min.1 + h);
                if !occupied.iter().any(|(a, c)| {
                    min.0 < c.0 + 1. && max.0 > a.0 - 1. && min.1 < c.1 + 1. && max.1 > a.1 - 1.
                }) && [min, max, (min.0, max.1), (max.0, min.1)]
                    .iter()
                    .all(|p| b.inside(*p))
                    && !keepouts.iter().any(|k| {
                        k.distance(&pcb_checks::Geometry::Polygon(vec![
                            min,
                            (max.0, min.1),
                            max,
                            (min.0, max.1),
                        ])) < b.rules.clearance
                    })
                {
                    let at = (min.0 - a.0, min.1 - a.1);
                    candidates.push(((at.0 - target.0).hypot(at.1 - target.1), at, (min, max)));
                }
            }
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let(_,at,bounds)=candidates.first().copied().with_context(||format!("Cannot place {reference}: footprint bounds {w:.2}×{h:.2} mm do not fit available board area"))?;
        b.place_footprint(i, at, 0.);
        occupied.push(bounds);
        placed.push((i, at));
    }
    b.dirty = true;
    Ok(
        json!({"placed":placed.iter().map(|(i,at)|json!({"reference":pcb::field(&b.root.items()[*i],"Reference"),"at_mm":at})).collect::<Vec<_>>()}),
    )
}

pub fn changes(before: Option<&Board>, after: &Board) -> Value {
    let old = before
        .map(|b| b.inspect(&json!({}))["objects"].clone())
        .unwrap_or(json!([]));
    let new = after.inspect(&json!({}));
    let map = |v: &Value| {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|o| o["uuid"].as_str().map(|id| (id.to_owned(), o.clone())))
            .collect::<BTreeMap<_, _>>()
    };
    let a = map(&old);
    let b = map(&new["objects"]);
    json!({"added":b.iter().filter(|(id,_)|!a.contains_key(*id)).map(|(_,o)|o).collect::<Vec<_>>(),"changed":b.iter().filter(|(id,o)|a.get(*id).is_some_and(|p|p!=*o)).map(|(_,o)|o).collect::<Vec<_>>(),"removed":a.keys().filter(|id|!b.contains_key(*id)).collect::<Vec<_>>()})
}
fn preview_path(id: &str) -> Result<PathBuf> {
    ensure!(
        uuid::Uuid::parse_str(id).is_ok(),
        "preview_id must be a UUID"
    );
    Ok(crate::catalog::directory()
        .join("pcb-previews")
        .join(format!("{id}.json")))
}
fn preview(e: &Engine, before: Option<&Board>, after: &Board, result: Value) -> Result<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let path = preview_path(&id)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    let receipt = json!({"base_revision":before.map(Board::revision),"base_file":before.and_then(|b|b.path.clone()),"schematic_revision":e.doc.as_ref().map(crate::journal::revision),"workspace":e.workspace,"candidate_path":after.path,"rules":after.rules,"schematic":after.schematic,"source":after.root.document_text(),"result":result});
    atomic_write(&path, &serde_json::to_vec(&receipt)?, false)?;
    let image = path.with_extension("png");
    crate::render::preview(&after.scene(None, true), &image, 1280)?;
    Ok(
        json!({"preview_id":id,"preview_image_path":image,"applied":false,"base_revision":receipt["base_revision"],"changes":changes(before,after),"can_commit":result["can_commit"],"failures":result["failures"],"verification":result["verification"],"results":result["results"]}),
    )
}
fn commit(e: &mut Engine, p: &Value) -> Result<Value> {
    let receipt: Value =
        serde_json::from_slice(&std::fs::read(preview_path(string(p, "preview_id")?)?)?)?;
    ensure!(
        receipt["result"]["can_commit"] == true,
        "PCB preview has failures; revise and preview again"
    );
    ensure!(
        receipt["base_revision"] == json!(e.board.as_ref().map(Board::revision))
            && receipt["base_file"] == json!(e.board.as_ref().and_then(|b| b.path.clone()))
            && receipt["schematic_revision"] == json!(e.doc.as_ref().map(crate::journal::revision))
            && receipt["workspace"] == json!(e.workspace),
        "PCB preview is stale or belongs to another project"
    );
    if let Some(b) = &e.board {
        ensure!(
            !b.disk_changed()?,
            "PCB saved file changed externally; reopen and preview again"
        );
    }
    let mut b = e.board.clone().unwrap_or(Board::new(80., 60.)?);
    b.root = parse(string(&receipt, "source")?)?;
    b.rules = serde_json::from_value(receipt["rules"].clone())?;
    b.schematic = receipt["schematic"].as_str().map(PathBuf::from);
    b.path = receipt["candidate_path"].as_str().map(PathBuf::from);
    b.dirty = true;
    apply(e, e.board.clone(), b, p, "pcb_commit")?;
    Ok(
        json!({"applied":true,"revision":board(e)?.revision(),"dirty":true,"preview_id":p["preview_id"]}),
    )
}
pub fn history_path(b: &Board) -> Option<PathBuf> {
    Some(
        b.path
            .as_ref()?
            .parent()?
            .join(".jenga/pcb-history")
            .join(format!(
                "{}.jsonl",
                b.path.as_ref()?.file_stem()?.to_string_lossy()
            )),
    )
}
pub fn record(before: Option<&Board>, after: &Board, actor: &str, method: &str) -> Result<()> {
    if let Some(path) = history_path(after) {
        std::fs::create_dir_all(path.parent().unwrap())?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        let value = json!({"before_revision":before.map(Board::revision),"after_revision":after.revision(),"before":before.map(|b|b.root.document_text()),"after":after.root.document_text(),"actor":actor,"operation":method,"timestamp_ms":crate::journal::now()});
        serde_json::to_writer(&mut file, &value)?;
        file.write_all(b"\n")?;
    }
    Ok(())
}
fn apply(
    e: &mut Engine,
    before: Option<Board>,
    mut after: Board,
    p: &Value,
    method: &str,
) -> Result<()> {
    if e.record_history {
        record(
            before.as_ref(),
            &after,
            p["actor"].as_str().unwrap_or("external"),
            method,
        )?;
    }
    if let Some(before) = before {
        after.undo.push(before.state());
        if after.undo.len() > 100 {
            after.undo.remove(0);
        }
    }
    after.redo.clear();
    e.board = Some(after);
    e.active_kind = "pcb".into();
    Ok(())
}
fn changes_since(b: &Board, revision: &str) -> Result<Value> {
    if b.revision() == revision {
        return Ok(json!({"revision":revision,"changes":{"added":[],"changed":[],"removed":[]}}));
    }
    let text =
        std::fs::read_to_string(history_path(b).context("Save a PCB path for revision history")?)
            .context("PCB revision is not in history")?;
    let entry = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["before_revision"] == revision || v["after_revision"] == revision)
        .context("Unknown PCB since_revision")?;
    let source = if entry["before_revision"] == revision {
        &entry["before"]
    } else {
        &entry["after"]
    };
    let mut old = b.clone();
    old.root = parse(
        source
            .as_str()
            .context("Revision precedes PCB creation; request full inspect")?,
    )?;
    Ok(json!({"revision":b.revision(),"changes":changes(Some(&old),b)}))
}
fn export(e: &Engine, p: &Value) -> Result<Value> {
    let b = board(e)?;
    let path = e.resolve(string(p, "path")?);
    let format = p["format"].as_str().unwrap_or("native");
    if path.exists() {
        if ["manufacturing", "gerbers", "drill", "svg", "pdf"].contains(&format) {
            ensure!(
                path.is_dir() && std::fs::read_dir(&path)?.next().is_none(),
                "Export destination folder must be empty"
            );
        } else {
            bail!("Export destination exists: {}", path.display());
        }
    }
    if format == "native" {
        ensure!(!path.exists(), "Native export destination exists");
        atomic_write(&path, b.root.document_text().as_bytes(), false)?;
        return Ok(json!({"path":path,"format":"native","revision":b.revision()}));
    }
    if format == "png" {
        crate::render::preview(
            &b.scene(None, p["ratsnest"] == true),
            &path,
            p["width"].as_u64().unwrap_or(1600).clamp(320, 4096) as u32,
        )?;
        return Ok(json!({"path":path,"format":"png"}));
    }
    if format == "bom" {
        let mut text = String::from("Reference,Value,Footprint,X_mm,Y_mm,Rotation_deg,Side\n");
        for f in b.root.children("footprint") {
            let esc = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
            let at = pcb::point(f, "at");
            text.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                esc(pcb::field(f, "Reference")),
                esc(pcb::field(f, "Value")),
                esc(f.value(1)),
                at.0,
                at.1,
                pcb::angle(f),
                pcb::layer(f)
            ));
        }
        atomic_write(&path, text.as_bytes(), false)?;
        return Ok(json!({"path":path,"format":"bom"}));
    }
    if format == "manufacturing" {
        let verification = verify_with_source(b, e.doc.as_ref(), &json!({"external":true}))?;
        ensure!(
            verification["passed"] == true || p["allow_unverified"] == true,
            "Manufacturing export blocked by verification: {}",
            verification
        );
        std::fs::create_dir_all(&path)?;
        let gerbers = pcb_backend::export(b, &path, "gerbers")?;
        let drill = pcb_backend::export(b, &path, "drill")?;
        let position = pcb_backend::export(b, &path.join("placement.csv"), "pos")?;
        export(e, &json!({"path":path.join("bom.csv"),"format":"bom"}))?;
        atomic_write(
            &path.join("verification.json"),
            &serde_json::to_vec_pretty(&verification)?,
            false,
        )?;
        return Ok(
            json!({"path":path,"verification":verification,"gerbers":gerbers,"drill":drill,"placement":position}),
        );
    }
    if matches!(format, "gerbers" | "drill") && p["allow_unverified"] != true {
        ensure!(
            verify_with_source(b, e.doc.as_ref(), &json!({"external":true}))?["passed"] == true,
            "PCB verification failed; manufacturing export requires corrections or explicit allow_unverified:true"
        );
    }
    pcb_backend::export(b, &path, format)
}

fn project_operation(e: &mut Engine, method: &str, p: &Value) -> Result<Value> {
    let b = board(e)?;
    ensure!(!b.dirty, "Save PCB before project maintenance");
    let path = b.path.clone().context("No saved PCB")?;
    let folder = path.parent().context("No project folder")?;
    ensure!(
        !e.doc
            .as_ref()
            .is_some_and(|d| d.dirty && d.path.as_ref().is_some_and(|p| p.starts_with(folder))),
        "Save the schematic before project maintenance"
    );
    if matches!(method, "pcb_rename_project" | "pcb_copy_project")
        && path.with_extension("kicad_sch").is_file()
    {
        let mut doc = Document::open(&path.with_extension("kicad_sch"))?;
        let name = string(p, "name")?;
        let old = path.clone();
        if method == "pcb_rename_project" {
            crate::projects::rename(
                &mut doc,
                &json!({"name":name,"kind":"project","move_folder":p["move_folder"]==true}),
            )?;
        } else {
            doc = crate::projects::copy_project(&doc, &e.resolve(string(p, "folder")?), name)?;
        }
        let new = doc.path.as_ref().unwrap().with_extension("kicad_pcb");
        e.board = Some(Board::open(&new)?);
        e.doc = Some(doc);
        let _ = crate::catalog::record(
            if method == "pcb_rename_project" {
                "rename"
            } else {
                "save_project_as"
            },
            Some(&old),
            Some(&new),
        );
        return Ok(
            json!({"file":new,"kind":"mixed_project","schematic":e.doc.as_ref().unwrap().path}),
        );
    }
    if method == "pcb_archive_project" {
        let archive = crate::catalog::directory()
            .join("trash")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&archive)?;
        let full = folder.file_name() == path.file_stem()
            && std::fs::read_dir(folder)?
                .flatten()
                .filter(|f| f.path().extension().is_some_and(|e| e == "kicad_pro"))
                .count()
                <= 1;
        let sources = if full {
            vec![folder.to_owned()]
        } else {
            [path.clone(), path.with_extension("kicad_pro")]
                .into_iter()
                .filter(|p| p.is_file())
                .collect()
        };
        let moves: Vec<_> = sources
            .iter()
            .map(|p| (p.clone(), archive.join(p.file_name().unwrap())))
            .collect();
        atomic_write(
            &archive.join("restore.json"),
            &serde_json::to_vec_pretty(&json!({"moves":moves,"project":path}))?,
            false,
        )?;
        let mut done = vec![];
        for (source, dest) in &moves {
            if let Err(err) = std::fs::rename(source, dest) {
                for (source, dest) in done.iter().rev() {
                    let _ = std::fs::rename(dest, source);
                }
                return Err(err.into());
            }
            done.push((source.clone(), dest.clone()));
        }
        let _ = crate::catalog::record("delete_project", Some(&path), None);
        e.board = None;
        if full
            || e.doc
                .as_ref()
                .is_some_and(|d| d.path.as_ref().is_some_and(|p| !p.is_file()))
        {
            e.doc = None;
        }
        return Ok(
            json!({"archived":true,"restore_manifest":archive.join("restore.json"),"recovery_folder":archive}),
        );
    }
    let name = string(p, "name")?;
    crate::projects::valid_name(name)?;
    if method == "pcb_rename_project" {
        let new = folder.join(format!("{name}.kicad_pcb"));
        ensure!(!new.exists(), "PCB rename destination exists");
        let old_pro = path.with_extension("kicad_pro");
        let new_pro = new.with_extension("kicad_pro");
        ensure!(!new_pro.exists(), "Project rename destination exists");
        std::fs::rename(&path, &new)?;
        if old_pro.exists() {
            if let Err(err) = std::fs::rename(&old_pro, &new_pro) {
                let _ = std::fs::rename(&new, &path);
                return Err(err.into());
            }
        }
        let oldmeta = Board::metadata_path(&path);
        let newmeta = Board::metadata_path(&new);
        if oldmeta.exists() {
            if let Err(err) = std::fs::rename(&oldmeta, &newmeta) {
                let _ = std::fs::rename(&new, &path);
                if new_pro.exists() {
                    let _ = std::fs::rename(&new_pro, &old_pro);
                }
                return Err(err.into());
            }
        }
        let mut renamed = Board::open(&new)?;
        renamed.dirty = true;
        renamed.save(&new)?;
        e.board = Some(renamed);
        let _ = crate::catalog::record("rename", Some(&path), Some(&new));
        return Ok(json!({"file":new,"renamed":true}));
    }
    let parent = e.resolve(string(p, "folder")?);
    let dest = if parent.file_name().is_some_and(|s| s == name) {
        parent
    } else {
        parent.join(name)
    };
    ensure!(!dest.exists(), "Project copy destination exists");
    fn copy(src: &Path, dst: &Path, depth: usize) -> Result<()> {
        ensure!(depth < 32, "Project nesting exceeds 32");
        std::fs::create_dir_all(dst)?;
        for file in std::fs::read_dir(src)? {
            let file = file?;
            let target = dst.join(file.file_name());
            if file.file_type()?.is_symlink() {
                bail!("Project copy refuses symlink {}", file.path().display());
            }
            if file.file_type()?.is_dir() {
                copy(&file.path(), &target, depth + 1)?;
            } else {
                std::fs::copy(file.path(), target)?;
            }
        }
        Ok(())
    }
    copy(folder, &dest, 0)?;
    let copied = dest.join(path.file_name().unwrap());
    let new = dest.join(format!("{name}.kicad_pcb"));
    if copied != new {
        std::fs::rename(&copied, &new)?;
    }
    let oldpro = dest.join(path.with_extension("kicad_pro").file_name().unwrap());
    if oldpro.is_file() {
        std::fs::rename(oldpro, new.with_extension("kicad_pro"))?;
    }
    let oldmeta = Board::metadata_path(&copied);
    let newmeta = Board::metadata_path(&new);
    if oldmeta != newmeta && oldmeta.exists() {
        std::fs::rename(&oldmeta, &newmeta)?;
    }
    let mut b = Board::open(&new)?;
    if let Some(schematic) = &b.schematic {
        if let Ok(relative) = schematic.strip_prefix(folder) {
            b.schematic = Some(dest.join(relative));
        }
    }
    b.dirty = true;
    b.save(&new)?;
    e.board = Some(b);
    e.active_kind = "pcb".into();
    let _ = crate::catalog::record("save_project_as", None, Some(&new));
    Ok(json!({"file":new,"copied":true,"original":path}))
}

fn flip_footprint(n: &mut Node) {
    fn flip(n: &mut Node) {
        if n.name() == "layer" {
            let old = n.value(1);
            if let Some(rest) = old.strip_prefix("F.") {
                n.items_mut()[1] = Node::string(format!("B.{rest}"));
            } else if let Some(rest) = old.strip_prefix("B.") {
                n.items_mut()[1] = Node::string(format!("F.{rest}"));
            }
        } else if n.name() == "layers" {
            for l in n.items_mut().iter_mut().skip(1) {
                let text = l.text().to_owned();
                if let Some(rest) = text.strip_prefix("F.") {
                    *l = Node::string(format!("B.{rest}"));
                } else if let Some(rest) = text.strip_prefix("B.") {
                    *l = Node::string(format!("F.{rest}"));
                }
            }
        } else if matches!(n.name(), "at" | "start" | "end" | "mid" | "center" | "xy") {
            if n.items().len() > 2 {
                n.items_mut()[2] = Node::atom(-n.number(2));
            }
            if n.name() == "at" && n.items().len() > 3 {
                n.items_mut()[3] = Node::atom(-n.number(3));
            }
        }
        if let Node::List(items) = n {
            for child in items.iter_mut().skip(1) {
                flip(child);
            }
        }
    }

    let r = pcb::angle(n);
    for child in n.items_mut().iter_mut().skip(2) {
        if child.name() != "at" {
            flip(child);
        }
    }
    for child in n
        .items_mut()
        .iter_mut()
        .filter(|n| matches!(n.name(), "pad" | "property" | "fp_text"))
    {
        let p = pcb::point(child, "at");
        let angle = pcb::angle(child) + 2. * r;
        child.set_child(pcb::node(
            "at",
            vec![Node::atom(p.0), Node::atom(p.1), Node::atom(angle)],
        ));
    }
}
