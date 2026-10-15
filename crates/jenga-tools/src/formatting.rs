//! Native text/field formatting shared by desktop and deterministic tools.
use anyhow::{Result, ensure};
use jenga_core::{document::Document, sexpr::Node};
use serde_json::{Value, json};

pub fn apply(n: &mut Node, a: &Value) -> Result<()> {
    let mut effects = n
        .child("effects")
        .cloned()
        .unwrap_or_else(|| Node::list("effects", vec![]));
    let mut font = effects
        .child("font")
        .cloned()
        .unwrap_or_else(|| Node::list("font", vec![]));
    if let Some(size) = a.get("size_mm") {
        let size = size
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("size_mm must be a number"))?;
        ensure!(
            size.is_finite() && (0.3..=20.).contains(&size),
            "Font size must be 0.3–20 mm"
        );
        font.set_child(Node::list("size", vec![Node::atom(size), Node::atom(size)]));
    }
    for key in ["bold", "italic"] {
        if let Some(value) = a.get(key) {
            let value = value
                .as_bool()
                .ok_or_else(|| anyhow::anyhow!("{key} must be boolean"))?;
            font.items_mut().retain(|v| v.text() != key);
            if value {
                font.items_mut().push(Node::atom(key));
            }
        }
    }
    effects.set_child(font);
    if let Some(align) = a.get("align").and_then(Value::as_str) {
        ensure!(
            ["left", "center", "right"].contains(&align),
            "Alignment must be left, center or right"
        );
        effects.items_mut().retain(|v| v.name() != "justify");
        if align != "center" {
            effects.set_child(Node::list("justify", vec![Node::atom(align)]));
        }
    }
    if let Some(hidden) = a.get("hidden").and_then(Value::as_bool) {
        effects.set_child(Node::list(
            "hide",
            vec![Node::atom(if hidden { "yes" } else { "no" })],
        ));
    }
    n.set_child(effects);
    let mut at = n
        .child("at")
        .cloned()
        .unwrap_or_else(|| Node::list("at", vec![Node::atom(0), Node::atom(0), Node::atom(0)]));
    if a.get("at").is_some() {
        let p = crate::engine::point(a, "at")?;
        at.items_mut()[1] = Node::atom(p.0);
        at.items_mut()[2] = Node::atom(p.1);
    }
    if let Some(angle) = a.get("angle").and_then(Value::as_f64) {
        ensure!(angle.is_finite(), "Invalid text angle");
        while at.items().len() < 4 {
            at.items_mut().push(Node::atom(0));
        }
        at.items_mut()[3] = Node::atom(angle.rem_euclid(360.));
    }
    if a.get("upright") == Some(&Value::Bool(true)) {
        while at.items().len() < 4 {
            at.items_mut().push(Node::atom(0));
        }
        let angle = at.number(3).rem_euclid(360.);
        at.items_mut()[3] = Node::atom(if angle > 90. && angle <= 270. {
            (angle + 180.).rem_euclid(360.)
        } else {
            angle
        });
    }
    n.set_child(at);
    Ok(())
}

/// Reannotate all units of a physical reference together; preserve assigned refs
/// by default, fill placeholders without colliding with existing references.
pub fn annotate(doc: &mut Document, a: &Value) -> Result<Value> {
    use std::collections::{HashMap, HashSet};
    let reset = a.get("reset").and_then(Value::as_bool).unwrap_or(false);
    ensure!(
        !doc.root
            .children("symbol")
            .any(
                |n| jenga_core::document::property(n, "Reference").ends_with('?')
                    && n.child("unit").is_some_and(|u| u.number(1) > 1.)
            ),
        "Unannotated multi-unit symbols require explicit shared references before automatic annotation; physical grouping is ambiguous"
    );
    let mut used: HashSet<String> = doc
        .root
        .children("symbol")
        .map(|n| jenga_core::document::property(n, "Reference").to_owned())
        .filter(|s| !s.ends_with('?') && !reset)
        .collect();
    let mut assigned = HashMap::new();
    let mut changes = vec![];
    for n in doc
        .root
        .items_mut()
        .iter_mut()
        .filter(|n| n.name() == "symbol")
    {
        let old = jenga_core::document::property(n, "Reference").to_owned();
        if old.starts_with('#') || (!reset && !old.ends_with('?')) {
            continue;
        }
        let unit = n.child("unit").map(|v| v.number(1)).unwrap_or(1.);
        let key = if old.ends_with('?') && unit <= 1. {
            n.child("uuid").map(|v| v.value(1)).unwrap_or("").to_owned()
        } else {
            old.clone()
        };
        let prefix: String = old
            .chars()
            .take_while(|c| !c.is_ascii_digit() && *c != '?')
            .collect();
        ensure!(!prefix.is_empty(), "Reference has no prefix: {old}");
        let new = assigned
            .entry(key)
            .or_insert_with(|| {
                let mut number = a.get("start").and_then(Value::as_u64).unwrap_or(1).max(1);
                loop {
                    let r = format!("{prefix}{number}");
                    if used.insert(r.clone()) {
                        break r;
                    }
                    number += 1;
                }
            })
            .clone();
        if let Some(p) = n
            .items_mut()
            .iter_mut()
            .find(|v| v.name() == "property" && v.value(1) == "Reference")
        {
            p.items_mut()[2] = Node::string(&new);
        }
        // Native instance paths also carry reference annotations.
        fn update(n: &mut Node, reference: &str) {
            if n.name() == "reference" {
                n.items_mut()[1] = Node::string(reference);
            } else if let Node::List(xs) = n {
                for x in xs {
                    update(x, reference)
                }
            }
        }
        if let Some(instances) = n.child_mut("instances") {
            update(instances, &new);
        }
        changes.push(json!({"old":old,"new":new,"unit":unit}));
    }
    doc.dirty |= !changes.is_empty();
    Ok(json!({"changes":changes}))
}
