//! Intent checks independent of ERC: reject unrelated net merges before commit.
use anyhow::{Context, Result, ensure};
use jenga_core::document::Document;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
type Pin = (String, String);
fn partition(doc: &Document) -> HashMap<Pin, usize> {
    let v = jenga_core::connectivity::inspect(doc);
    let mut map = HashMap::new();
    if let Some(nets) = v["nets"].as_array() {
        for (i, net) in nets.iter().enumerate() {
            for p in net["pins"].as_array().into_iter().flatten() {
                map.insert(
                    (
                        p["uuid"].as_str().unwrap_or("").into(),
                        p["number"].as_str().unwrap_or("").into(),
                    ),
                    i,
                );
            }
        }
    }
    map
}
fn key(doc: &Document, v: &Value) -> Result<Pin> {
    let i = crate::engine::target(doc, v)?;
    let n = &doc.root.items()[i];
    Ok((
        n.child("uuid").context("No UUID")?.value(1).into(),
        crate::engine::string(v, "number")?.into(),
    ))
}
pub fn route_change(before: &Document, after: &Document, from: &Value, to: &Value) -> Result<()> {
    let old = partition(before);
    let new = partition(after);
    let a = *old
        .get(&key(before, from)?)
        .context("Source pin not found")?;
    let b = *old
        .get(&key(before, to)?)
        .context("Destination pin not found")?;
    let mut incoming: HashMap<usize, HashSet<usize>> = HashMap::new();
    for (p, group) in old {
        if let Some(now) = new.get(&p) {
            incoming.entry(*now).or_default().insert(group);
        }
    }
    for groups in incoming.values() {
        ensure!(
            groups.len() <= 1 || groups.iter().all(|g| *g == a || *g == b),
            "Route would merge an unrelated electrical net; no edits committed"
        );
    }
    Ok(())
}
pub fn verify(doc: &Document, p: &Value) -> Result<Value> {
    let nets = partition(doc);
    let mut findings = vec![];
    for (field, wanted) in [
        ("expected_connections", true),
        ("forbidden_connections", false),
    ] {
        for pair in p.get(field).and_then(Value::as_array).into_iter().flatten() {
            let from = pair.get("from").context("Connection needs from")?;
            let to = pair.get("to").context("Connection needs to")?;
            let a = nets.get(&key(doc, from)?).context("Source pin missing")?;
            let b = nets
                .get(&key(doc, to)?)
                .context("Destination pin missing")?;
            if (*a == *b) != wanted {
                findings.push(
                    json!({"from":from,"to":to,"expected_connected":wanted,"connected":a==b}),
                );
            }
        }
    }
    Ok(
        json!({"passed":findings.is_empty(),"findings":findings,"revision":crate::journal::revision(doc)}),
    )
}
pub fn avoid_points(doc: &Document) -> Vec<(f64, f64)> {
    doc.root
        .items()
        .iter()
        .filter(|n| {
            matches!(
                n.name(),
                "label" | "global_label" | "hierarchical_label" | "junction" | "no_connect"
            )
        })
        .map(|n| {
            let (x, y, _) = jenga_core::document::position(n);
            (x, y)
        })
        .collect()
}
