//! Compact engineering workflows shared by the desktop and headless engine.
use anyhow::{Context, Result, ensure};
use jenga_core::{
    checks::on_segment,
    document::{Document, position},
    scene::{Scene, points},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

pub fn block(p: &Value) -> Result<Value> {
    let pairs = p["connections"]
        .as_array()
        .context("connect_block requires connections: [{from,to}]")?;
    ensure!(
        !pairs.is_empty() && pairs.len() <= 1000,
        "Supply 1–1000 connections"
    );
    let mut request = p.clone();
    request["actions"] = json!(
        pairs
            .iter()
            .map(|pair| {
                let mut a = pair.clone();
                a["op"] = json!("connect_pins");
                if p.get("reserved_corridors").is_some() {
                    a["reserved_corridors"] = p["reserved_corridors"].clone();
                }
                a
            })
            .collect::<Vec<_>>()
    );
    let mut expected = p["expected_connections"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    expected.extend(pairs.iter().cloned());
    request["expected_connections"] = json!(expected);
    Ok(request)
}

fn objects(doc: &Document) -> BTreeMap<String, Value> {
    crate::engine::inspect(doc)["objects"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| Some((v["uuid"].as_str()?.to_owned(), v.clone())))
        .collect()
}
pub fn changes(before: &Document, after: &Document) -> Value {
    let a = objects(before);
    let b = objects(after);
    let old_nets = jenga_core::connectivity::inspect(before);
    let new_nets = jenga_core::connectivity::inspect(after);
    json!({"added":b.iter().filter(|(k,_)|!a.contains_key(*k)).map(|(_,v)|v).collect::<Vec<_>>(),
        "changed":b.iter().filter(|(k,v)|a.get(*k).is_some_and(|old|old!=*v)).map(|(_,v)|v).collect::<Vec<_>>(),
        "removed":a.keys().filter(|k|!b.contains_key(*k)).collect::<Vec<_>>(),
        "affected_nets":new_nets["nets"].as_array().unwrap().iter().filter(|n|!old_nets["nets"].as_array().unwrap().contains(n)).collect::<Vec<_>>()})
}
pub fn inspect(doc: &Document, p: &Value) -> Result<Value> {
    let mut v = crate::engine::inspect(doc);
    if let Some(revision) = p["since_revision"].as_str() {
        if revision == crate::journal::revision(doc) {
            v["changes"] = json!({"added":[],"changed":[],"removed":[]});
        } else {
            let path = crate::journal::path(doc).context("No history")?;
            let text = std::fs::read_to_string(path)
                .context("Revision is not in this document's history")?;
            let entry = text
                .lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .find(|e| {
                    e.get("resource").is_none()
                        && (e["before_revision"] == revision || e["after_revision"] == revision)
                })
                .context("Unknown since_revision; request a full inspect")?;
            let source = if entry["before_revision"] == revision {
                &entry["before"]
            } else {
                &entry["after"]
            };
            let mut old = doc.clone();
            old.root =
                jenga_core::sexpr::parse(source.as_str().context("Missing historical source")?)?;
            v["changes"] = changes(&old, doc);
        }
        v.as_object_mut().unwrap().remove("objects");
    } else {
        let selectors = |key: &str| {
            p[key]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect::<HashSet<_>>())
        };
        let refs = selectors("references");
        let ids = selectors("uuids");
        let kinds = selectors("kinds");
        v["objects"].as_array_mut().unwrap().retain(|o| {
            refs.as_ref()
                .is_none_or(|s| s.contains(o["reference"].as_str().unwrap_or("")))
                && ids
                    .as_ref()
                    .is_none_or(|s| s.contains(o["uuid"].as_str().unwrap_or("")))
                && kinds
                    .as_ref()
                    .is_none_or(|s| s.contains(o["kind"].as_str().unwrap_or("")))
        });
        if p["include_pins"] == false {
            for o in v["objects"].as_array_mut().unwrap() {
                o.as_object_mut().unwrap().remove("pins");
            }
        }
    }
    if p["nets"] == true {
        let mut nets = jenga_core::connectivity::inspect(doc)["nets"].clone();
        if let Some(names) = p["net_names"].as_array() {
            nets.as_array_mut().unwrap().retain(|n| {
                n["names"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|s| names.contains(s))
            });
        }
        v["nets"] = nets;
    }
    Ok(v)
}

pub fn verify(doc: &Document, p: &Value) -> Result<Value> {
    let intent = crate::net_guard::verify(doc, p)?;
    let visible = if p["require_visible_wires"] == true {
        let mut physical = doc.clone();
        physical
            .root
            .items_mut()
            .retain(|n| !matches!(n.name(), "label" | "global_label" | "hierarchical_label"));
        Some(crate::net_guard::verify(&physical, p)?)
    } else {
        None
    };
    let issues = crate::engine::findings(doc);
    let scene = Scene::from_document(doc);
    let wires: Vec<_> = doc
        .root
        .children("wire")
        .filter_map(|n| {
            let pts = points(n, "pts");
            (pts.len() == 2).then(|| (n, pts))
        })
        .collect();
    let anchors = crate::net_guard::avoid_points(doc);
    let mut dangling = vec![];
    for (i, (n, pts)) in wires.iter().enumerate() {
        for at in pts {
            if !scene
                .pins
                .iter()
                .any(|pin| (pin.at.0 - at.0).abs() < 1e-6 && (pin.at.1 - at.1).abs() < 1e-6)
                && !anchors.contains(at)
                && !wires
                    .iter()
                    .enumerate()
                    .any(|(j, (_, other))| j != i && on_segment(*at, other[0], other[1]))
            {
                dangling.push(json!({"uuid":n.child("uuid").map(|n|n.value(1)),"at_mm":at}));
            }
        }
    }
    let warnings = doc.warnings();
    let roundtrip =
        jenga_core::sexpr::parse(&doc.root.document_text()).is_ok_and(|n| n == doc.root);
    let passed = intent["passed"] == true
        && visible.as_ref().is_none_or(|v| v["passed"] == true)
        && dangling.is_empty()
        && scene.missing.is_empty()
        && warnings.is_empty()
        && roundtrip
        && !issues
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["severity"] == "error");
    Ok(
        json!({"revision":crate::journal::revision(doc),"passed":passed,"scope":"single_sheet",
        "erc":{"issues":issues},"connections":intent,"visible_wire_connections":visible,"dangling_wires":dangling,
        "kicad":{"native_roundtrip":roundtrip,"missing_symbols":scene.missing,"warnings":warnings,"external_cli_checked":false},
        "page_semantics":"Independent drawings have separate electrical scope. Hierarchical pages require explicit sheet pins; cross-sheet electrical analysis is not implemented."}),
    )
}

pub fn cleanup(doc: &mut Document) -> Result<Value> {
    let before = doc.clone();
    let mut seen = vec![];
    let mut remove = vec![];
    for (i, n) in doc.root.items().iter().enumerate() {
        if n.name() == "wire" {
            let p = points(n, "pts");
            if p.len() == 2 {
                if p[0] == p[1]
                    || seen
                        .iter()
                        .any(|q: &Vec<(f64, f64)>| *q == p || (q[0] == p[1] && q[1] == p[0]))
                {
                    remove.push(i);
                } else {
                    seen.push(p);
                }
            }
        }
        if n.name() == "junction" {
            let (x, y, _) = position(n);
            let at = (x, y);
            // Only unattached dots are orphaned. Keep crossing and branch dots.
            if !doc.root.children("wire").any(|w| {
                let p = points(w, "pts");
                p.len() == 2 && on_segment(at, p[0], p[1])
            }) && !Scene::from_document(doc).pins.iter().any(|p| p.at == at)
            {
                remove.push(i);
            }
        }
    }
    for i in remove.iter().rev() {
        doc.remove(*i);
    }
    let mut merged = 0;
    loop {
        let scene = Scene::from_document(doc);
        let anchors = crate::net_guard::avoid_points(doc);
        let wires: Vec<_> = doc
            .root
            .items()
            .iter()
            .enumerate()
            .filter(|(_, n)| n.name() == "wire")
            .filter_map(|(i, n)| {
                let p = points(n, "pts");
                (p.len() == 2).then_some((i, p))
            })
            .collect();
        let mut pair = None;
        'search: for (a, (i, p)) in wires.iter().enumerate() {
            for (j, q) in wires.iter().skip(a + 1) {
                for shared in p {
                    if !q.contains(shared) {
                        continue;
                    }
                    let x = if p[0] == *shared { p[1] } else { p[0] };
                    let y = if q[0] == *shared { q[1] } else { q[0] };
                    if x == y
                        || !on_segment(*shared, x, y)
                        || anchors.contains(shared)
                        || scene.pins.iter().any(|p| p.at == *shared)
                    {
                        continue;
                    }
                    if wires
                        .iter()
                        .any(|(k, r)| k != i && k != j && on_segment(*shared, r[0], r[1]))
                    {
                        continue;
                    }
                    pair = Some((*i, *j, x, y));
                    break 'search;
                }
            }
        }
        let Some((i, j, x, y)) = pair else {
            break;
        };
        doc.root.items_mut()[i].set_child(jenga_core::sexpr::Node::list(
            "pts",
            vec![
                jenga_core::sexpr::Node::list(
                    "xy",
                    vec![
                        jenga_core::sexpr::Node::atom(x.0),
                        jenga_core::sexpr::Node::atom(x.1),
                    ],
                ),
                jenga_core::sexpr::Node::list(
                    "xy",
                    vec![
                        jenga_core::sexpr::Node::atom(y.0),
                        jenga_core::sexpr::Node::atom(y.1),
                    ],
                ),
            ],
        ));
        doc.remove(j);
        merged += 1;
    }
    ensure!(
        crate::net_guard::signature(&before) == crate::net_guard::signature(doc),
        "Cleanup changed connectivity; batch rolled back"
    );
    Ok(json!({"removed":remove.len(),"segments_merged":merged,"changes":changes(&before,doc)}))
}

pub fn preview_path(id: &str) -> Result<std::path::PathBuf> {
    ensure!(
        uuid::Uuid::parse_str(id).is_ok(),
        "preview_id must be a UUID"
    );
    Ok(crate::catalog::directory()
        .join("previews")
        .join(format!("{id}.json")))
}
pub fn store_preview(before: &Document, after: &Document, result: Value) -> Result<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let path = preview_path(&id)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    let image_path = path.with_extension("png");
    crate::render::preview(&Scene::from_document(after), &image_path, 1280)?;
    let receipt = json!({"preview_id":id,"base_revision":crate::journal::revision(before),"root_uuid":before.root_uuid(),"file":before.path,"source":after.root.document_text(),"result":result});
    jenga_core::document::atomic_write(&path, &serde_json::to_vec(&receipt)?, false)?;
    Ok(
        json!({"preview_id":id,"preview_image_path":image_path,"base_revision":crate::journal::revision(before),"applied":false,"changes":changes(before,after),"can_commit":receipt["result"]["can_commit"],"failures":receipt["result"]["failures"],"verification":receipt["result"]["verification"],"results":receipt["result"]["results"]}),
    )
}
