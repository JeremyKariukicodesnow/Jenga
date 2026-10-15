use anyhow::{Context, Result, ensure};
use jenga_core::{
    checks::on_segment,
    document::{Document, position},
    scene::{Scene, points},
    sexpr::Node,
};
use jenga_libraries::Libraries;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

pub fn replace(doc: &mut Document, libs: &mut Libraries, a: &Value) -> Result<Value> {
    let before = doc.clone();
    let i = crate::engine::target(doc, a)?;
    ensure!(
        doc.root.items()[i].name() == "symbol",
        "Target must be a symbol"
    );
    let instance = &doc.root.items()[i];
    let uuid = instance
        .child("uuid")
        .context("Symbol UUID missing")?
        .value(1)
        .to_owned();
    let id = if a["op"] == "replace_symbol" {
        crate::engine::string(a, "library_id")?.to_owned()
    } else {
        instance
            .child("lib_id")
            .context("Symbol library ID missing")?
            .value(1)
            .to_owned()
    };
    let unit = a["unit"].as_u64().unwrap_or_else(|| {
        instance
            .child("unit")
            .map(|n| n.number(1) as u64)
            .unwrap_or(1)
    });
    let mut def = libs.symbol(&id)?;
    let max = def
        .children("symbol")
        .filter_map(|s| s.value(1).rsplit('_').nth(1)?.parse::<u64>().ok())
        .max()
        .unwrap_or(1)
        .max(1);
    ensure!((1..=max).contains(&unit), "{} has no unit {}", id, unit);
    // Keep other instances' embedded geometry stable, even when refreshing one instance.
    let alias = format!("{id}__{}", uuid.replace('-', ""));
    def.items_mut()[1] = Node::string(&alias);
    let embedded = doc
        .root
        .child_mut("lib_symbols")
        .context("Missing embedded table")?;
    if let Some(existing) = embedded
        .items_mut()
        .iter_mut()
        .find(|n| n.name() == "symbol" && n.value(1) == alias)
    {
        *existing = def;
    } else {
        embedded.items_mut().push(def);
    }
    let n = &mut doc.root.items_mut()[i];
    n.set_child(Node::list("lib_id", vec![Node::string(&id)]));
    n.set_child(Node::list("lib_name", vec![Node::string(&alias)]));
    n.set_child(Node::list("unit", vec![Node::atom(unit)]));
    n.items_mut().retain(|n| n.name() != "pin");
    let old_scene = Scene::from_document(&before);
    let new_scene = Scene::from_document(doc);
    let old: Vec<_> = old_scene.pins.iter().filter(|p| p.object == i).collect();
    let new: Vec<_> = new_scene.pins.iter().filter(|p| p.object == i).collect();
    ensure!(
        !new.is_empty() || old.is_empty(),
        "Replacement {} unit {} has no pins",
        id,
        unit
    );
    let explicit = a["pin_map"].as_object();
    if let Some(map) = explicit {
        for key in map.keys() {
            ensure!(
                old.iter().any(|p| &p.number == key),
                "pin_map names unknown old pin {key}"
            );
        }
    }
    let mut used = HashSet::new();
    let mut mapping = BTreeMap::new();
    let mut routes = vec![];
    for p in &old {
        let number = explicit
            .and_then(|m| m.get(&p.number))
            .map(|v| {
                v.as_str()
                    .context("pin_map values must be new physical pin numbers")
            })
            .transpose()?;
        let candidates: Vec<_> = if let Some(number) = number {
            new.iter().filter(|q| q.number == number).copied().collect()
        } else {
            let same: Vec<_> = new
                .iter()
                .filter(|q| q.number == p.number)
                .copied()
                .collect();
            if same.is_empty() && !p.name.is_empty() {
                new.iter().filter(|q| q.name == p.name).copied().collect()
            } else {
                same
            }
        };
        ensure!(
            candidates.len() == 1,
            "Ambiguous or missing mapping for {} pin {} ({:?}); supply pin_map with one exact new pin number",
            p.reference,
            p.number,
            p.name
        );
        let q = candidates[0];
        ensure!(
            used.insert(q.number.clone()),
            "Multiple old pins map to new pin {}; specify a one-to-one pin_map",
            q.number
        );
        mapping.insert(p.number.clone(), q.number.clone());
        if p.at == q.at {
            continue;
        }
        let attached = before.root.children("wire").any(|w| {
            let pts = points(w, "pts");
            pts.len() == 2 && on_segment(p.at, pts[0], pts[1])
        }) || before.root.items().iter().any(|n| {
            matches!(n.name(), "label" | "global_label" | "hierarchical_label") && {
                let (x, y, _) = position(n);
                (x, y) == p.at
            }
        }) || old_scene
            .pins
            .iter()
            .any(|other| other.object != i && other.at == p.at);
        for n in doc.root.items_mut() {
            if n.name() == "no_connect" {
                let (x, y, angle) = position(n);
                if (x, y) == p.at {
                    n.set_child(jenga_core::document::at(q.at.0, q.at.1, angle));
                }
            }
        }
        if attached {
            let route = crate::routing::route(doc, &Scene::from_document(doc), q.at, p.at, true)
                .with_context(|| {
                    format!(
                        "Cannot preserve {}.{} → {}.{} at {:?} → {:?}",
                        p.reference, p.number, p.reference, q.number, p.at, q.at
                    )
                })?;
            for segment in route.windows(2) {
                doc.add_wire(segment[0], segment[1]);
            }
            routes.push(json!({"old_pin":p.number,"new_pin":q.number,"route_mm":route}));
        }
    }
    // Project both partitions onto the original pins and labels. Any split or new
    // merge (including a moved unused pin touching a net) rejects the whole edit.
    let project = |d: &Document, remap: bool| {
        let nets = jenga_core::connectivity::inspect(d);
        let oldnets = jenga_core::connectivity::inspect(&before);
        let known: HashSet<_> = oldnets["nets"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|n| n["pins"].as_array().unwrap())
            .map(|p| {
                (
                    p["uuid"].as_str().unwrap().to_owned(),
                    p["number"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let mut groups: Vec<Vec<String>> = nets["nets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                let mut keys = vec![];
                for p in n["pins"].as_array().unwrap() {
                    let u = p["uuid"].as_str().unwrap();
                    let mut num = p["number"].as_str().unwrap().to_owned();
                    if remap && u == uuid {
                        if let Some((old, _)) = mapping.iter().find(|(_, new)| **new == num) {
                            num = old.clone();
                        } else {
                            continue;
                        }
                    }
                    if known.contains(&(u.to_owned(), num.clone())) {
                        keys.push(format!("{u}:{num}"));
                    }
                }
                keys.extend(
                    n["names"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| format!("name:{s}")),
                );
                keys.sort();
                keys.dedup();
                keys
            })
            .filter(|g| !g.is_empty())
            .collect();
        groups.sort();
        groups
    };
    ensure!(
        project(&before, false) == project(doc, true),
        "Replacement would split or short existing nets; no edits committed. Review pin_map and symbol placement"
    );
    crate::net_guard::named_merges(&before, doc)?;
    doc.dirty = true;
    Ok(json!({"uuid":uuid,"library_id":id,"pin_map":mapping,"preserved_routes":routes}))
}
