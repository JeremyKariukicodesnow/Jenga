//! Intent checks independent of ERC: reject unrelated net merges before commit.
use anyhow::{Context, Result, ensure};
use jenga_core::document::Document;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
type Pin = (String, String);
pub fn signature(doc: &Document) -> Vec<Vec<String>> {
    let nets = jenga_core::connectivity::inspect(doc);
    let mut groups: Vec<_> = nets["nets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            let mut keys: Vec<String> = n["pins"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| format!("pin:{}:{}", p["uuid"], p["number"]))
                .collect();
            keys.extend(
                n["names"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| format!("name:{n}")),
            );
            keys.sort();
            keys.dedup();
            keys
        })
        .filter(|g| !g.is_empty())
        .collect();
    groups.sort();
    groups
}
/// Preview must name mergers outside the caller's stated connection intent.
pub fn unintended_merges(
    before: &Document,
    after: &Document,
    request: &Value,
) -> Result<Vec<Value>> {
    let old = partition(before);
    let new = partition(after);
    let count = old.values().copied().max().map(|n| n + 1).unwrap_or(0);
    let mut parents: Vec<usize> = (0..count).collect();
    fn root(p: &[usize], mut i: usize) -> usize {
        while p[i] != i {
            i = p[i];
        }
        i
    }
    let mut pairs = request["expected_connections"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    pairs.extend(
        request["actions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|a| a["op"] == "connect_pins")
            .cloned(),
    );
    for pair in pairs {
        let from = key(before, &pair["from"]);
        let to = key(before, &pair["to"]);
        if let (Ok(a), Ok(b)) = (from, to) {
            if let (Some(a), Some(b)) = (old.get(&a), old.get(&b)) {
                let a = root(&parents, *a);
                let b = root(&parents, *b);
                parents[b] = a;
            }
        }
    }
    let mut incoming: HashMap<usize, HashSet<usize>> = HashMap::new();
    for (pin, group) in old {
        if let Some(now) = new.get(&pin) {
            incoming
                .entry(*now)
                .or_default()
                .insert(root(&parents, group));
        }
    }
    let nets = jenga_core::connectivity::inspect(after);
    let mut findings = vec![];
    for (now, groups) in incoming {
        if groups.len() > 1 {
            findings.push(json!({"kind":"unintended_net_merge","net":nets["nets"][now],"previous_distinct_net_count":groups.len(),"suggestion":"Provide expected_connections for intentional joins, or correct the proposed wire"}));
        }
    }
    Ok(findings)
}
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
            let result = (|| -> Result<bool> {
                let from = pair.get("from").context("Connection needs from")?;
                let to = pair.get("to").context("Connection needs to")?;
                let a = nets
                    .get(&key(doc, from)?)
                    .with_context(|| format!("Source pin missing: {from}"))?;
                let b = nets
                    .get(&key(doc, to)?)
                    .with_context(|| format!("Destination pin missing: {to}"))?;
                Ok(a == b)
            })();
            match result {
            Ok(connected) if connected != wanted => {
                findings.push(
                    json!({"from":pair["from"],"to":pair["to"],"expected_connected":wanted,"connected":connected}),
                );
            },
            Err(e)=>findings.push(json!({"from":pair["from"],"to":pair["to"],"expected_connected":wanted,"error":format!("{e:#}")})),
            _=>{},
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

/// Explicitly labelled nets may only be merged with informed caller intent.
pub fn named_merges(before: &Document, after: &Document) -> Result<()> {
    let prior = jenga_core::connectivity::inspect(before);
    let next = jenga_core::connectivity::inspect(after);
    let old: Vec<HashSet<String>> = prior["nets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            n["names"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .collect();
    for net in next["nets"].as_array().unwrap() {
        let names: HashSet<String> = net["names"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        ensure!(
            names.len() < 2 || old.iter().any(|n| names.is_subset(n)),
            "Edit would join differently named nets {:?}; no edits committed. Inspect connectivity/diagnose and fix endpoint or label placement. Set allow_named_net_merges:true only for intentional aliases",
            names
        );
    }
    Ok(())
}
pub fn trace(doc: &Document, p: &Value) -> Result<Value> {
    let v = jenga_core::connectivity::inspect(doc);
    let a = key(doc, p.get("from").context("from required")?)?;
    let b = p.get("to").map(|p| key(doc, p)).transpose()?;
    let net = v["nets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| {
            n["pins"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["uuid"].as_str() == Some(&a.0) && p["number"].as_str() == Some(&a.1))
        })
        .context("Source pin missing")?;
    let connected = b.as_ref().map(|b| {
        net["pins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["uuid"].as_str() == Some(&b.0) && p["number"].as_str() == Some(&b.1))
    });
    Ok(
        json!({"revision":crate::journal::revision(doc),"connected":connected,"net":net,"note":"Actual electrical membership includes labels, pin contacts and T junctions; wire_uuids identify involved segments. This is not an endpoint-only graph."}),
    )
}
