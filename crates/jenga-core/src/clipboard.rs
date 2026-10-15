//! Native KiCad clipboard fragments: embedded definitions, geometry and fresh instance identity.
use crate::{
    document::{Document, editable, id, position, property},
    scene::points,
    sexpr::{Node, parse},
};
use anyhow::{Context, Result, ensure};
use std::collections::{HashMap, HashSet};

pub fn copy(doc: &Document, uuids: Option<&[String]>) -> Result<String> {
    let nodes: Vec<_> = doc
        .root
        .items()
        .iter()
        .filter(|n| {
            editable(n.name())
                && uuids.is_none_or(|ids| {
                    n.child("uuid")
                        .is_some_and(|u| ids.iter().any(|s| s == u.value(1)))
                })
        })
        .cloned()
        .collect();
    ensure!(
        !nodes.is_empty(),
        "Select objects or copy the whole schematic"
    );
    if let Some(ids) = uuids {
        ensure!(
            ids.len() == nodes.len(),
            "Selection contains missing or duplicate UUIDs"
        );
    }
    let ids: HashSet<_> = nodes
        .iter()
        .filter(|n| n.name() == "symbol")
        .filter_map(|n| {
            n.child("lib_name")
                .or_else(|| n.child("lib_id"))
                .map(|i| i.value(1))
        })
        .collect();
    let defs = ids
        .iter()
        .map(|id| {
            doc.embedded(id)
                .cloned()
                .with_context(|| format!("Missing embedded definition {id}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut fragment = vec![Node::list("lib_symbols", defs).encoded()];
    fragment.extend(nodes.iter().map(Node::encoded));
    // KiCad's selection serializer emits sibling expressions, without a document header.
    Ok(fragment.join("\n"))
}
fn fresh_ids(n: &mut Node) {
    if n.name() == "uuid" {
        *n = id();
        return;
    }
    if let Node::List(items) = n {
        for child in items {
            fresh_ids(child);
        }
    }
}
fn rename_body(n: &mut Node, old: &str, new: &str) {
    if n.name() == "symbol" && (n.value(1) == old || n.value(1).starts_with(&format!("{old}_"))) {
        let suffix = n.value(1)[old.len()..].to_owned();
        n.items_mut()[1] = Node::string(format!("{new}{suffix}"));
    }
    if let Node::List(items) = n {
        for child in items {
            rename_body(child, old, new);
        }
    }
}
pub fn paste(doc: &mut Document, text: &str, at: Option<(f64, f64)>) -> Result<Vec<String>> {
    ensure!(text.len() <= 32 * 1024 * 1024, "Clipboard exceeds 32 MB");
    let source = if text.trim_start().starts_with("(kicad_sch") {
        parse(text)?
    } else {
        parse(&format!("(kicad_sch {text})"))?
    };
    ensure!(
        source.name() == "kicad_sch",
        "Expected native KiCad schematic or clipboard fragment"
    );
    for n in source.items().iter().skip(1) {
        ensure!(
            editable(n.name())
                || matches!(
                    n.name(),
                    "lib_symbols"
                        | "version"
                        | "generator"
                        | "generator_version"
                        | "uuid"
                        | "paper"
                        | "title_block"
                        | "sheet_instances"
                        | "embedded_fonts"
                ),
            "Clipboard object '{}' is not supported; paste supported flat schematic objects only",
            n.name()
        );
    }
    let mut nodes: Vec<_> = source
        .items()
        .iter()
        .filter(|n| editable(n.name()))
        .cloned()
        .collect();
    for n in nodes.iter_mut().filter(|n| n.name() == "symbol") {
        if let Some(alias) = n.child("lib_name").map(|n| n.value(1).to_owned()) {
            n.set_child(Node::list("lib_id", vec![Node::string(alias)]));
            n.items_mut().retain(|n| n.name() != "lib_name");
        }
    }
    ensure!(
        !nodes.is_empty() && nodes.len() <= 10000,
        "Clipboard must contain 1–10000 supported objects"
    );
    let mut candidate = doc.clone();
    if candidate.root.child("lib_symbols").is_none() {
        candidate.root.set_child(Node::list("lib_symbols", vec![]));
    }
    let mut remap = HashMap::new();
    for n in nodes.iter().filter(|n| n.name() == "symbol") {
        let lib = n
            .child("lib_id")
            .context("Symbol missing lib_id")?
            .value(1)
            .to_owned();
        if remap.contains_key(&lib) {
            continue;
        }
        let def = source
            .child("lib_symbols")
            .and_then(|l| l.children("symbol").find(|s| s.value(1) == lib))
            .or_else(|| candidate.embedded(&lib))
            .context("Clipboard symbol missing embedded definition")?
            .clone();
        let mut name = lib.clone();
        let mut serial = 2;
        while candidate.embedded(&name).is_some_and(|old| old != &def) {
            name = format!("{lib}_jenga{serial}");
            serial += 1;
        }
        if candidate.embedded(&name).is_none() {
            let mut d = def;
            rename_body(&mut d, &lib, &name);
            candidate
                .root
                .child_mut("lib_symbols")
                .unwrap()
                .items_mut()
                .push(d);
        }
        remap.insert(lib, name);
    }
    let mut used: HashSet<String> = candidate
        .root
        .children("symbol")
        .map(|n| property(n, "Reference").to_owned())
        .collect();
    let mut references = HashMap::new();
    let mut coords = vec![];
    for n in &nodes {
        if n.child("at").is_some() {
            let p = position(n);
            coords.push((p.0, p.1));
        }
        coords.extend(points(n, "pts"));
    }
    ensure!(
        coords
            .iter()
            .all(|p| p.0.is_finite() && p.1.is_finite() && p.0.abs() < 1e6 && p.1.abs() < 1e6),
        "Invalid clipboard coordinates"
    );
    let low = (
        coords.iter().map(|p| p.0).fold(f64::INFINITY, f64::min),
        coords.iter().map(|p| p.1).fold(f64::INFINITY, f64::min),
    );
    let anchor = ((low.0 / 1.27).floor() * 1.27, (low.1 / 1.27).floor() * 1.27);
    let delta = at
        .map(|p| (p.0 - anchor.0, p.1 - anchor.1))
        .unwrap_or((10.16, 10.16));
    ensure!(
        delta.0.is_finite() && delta.1.is_finite(),
        "Invalid paste position"
    );
    let mut ids = vec![];
    for n in &mut nodes {
        fresh_ids(n);
        if n.child("uuid").is_none() {
            n.set_child(id());
        }
        if n.name() == "symbol" {
            let lib = n.child("lib_id").unwrap().value(1).to_owned();
            n.set_child(Node::list("lib_id", vec![Node::string(&remap[&lib])]));
            let old = property(n, "Reference").to_owned();
            let key = (old.clone(), lib);
            let new = references
                .entry(key)
                .or_insert_with(|| {
                    let prefix = old.trim_end_matches(|c: char| c.is_ascii_digit() || c == '?');
                    let prefix = if prefix.is_empty() { "U" } else { prefix };
                    let mut num = 1;
                    while used.contains(&format!("{prefix}{num}")) {
                        num += 1;
                    }
                    let new = format!("{prefix}{num}");
                    used.insert(new.clone());
                    new
                })
                .clone();
            if let Some(index) = n
                .items()
                .iter()
                .position(|p| p.name() == "property" && p.value(1) == "Reference")
            {
                n.items_mut()[index].items_mut()[2] = Node::string(&new);
            }
            let unit = n
                .child("unit")
                .map(|u| u.value(1))
                .unwrap_or("1")
                .to_owned();
            n.set_child(Node::list(
                "instances",
                vec![Node::list(
                    "project",
                    vec![
                        Node::string(candidate.project_name()),
                        Node::list(
                            "path",
                            vec![
                                Node::string(format!("/{}", candidate.root_uuid())),
                                Node::list("reference", vec![Node::string(new)]),
                                Node::list("unit", vec![Node::atom(unit)]),
                            ],
                        ),
                    ],
                )],
            ));
        }
        ids.push(n.child("uuid").unwrap().value(1).to_owned());
        let index = candidate.root.items().len();
        candidate.root.items_mut().push(n.clone());
        candidate.translate(index, delta.0, delta.1);
    }
    candidate.normalize_embedded_names();
    *doc = candidate;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_multiunit_identity_and_isolates_definition_conflicts() {
        let def=parse("(symbol \"Part\" (property \"Reference\" \"U\" (at 0 0 0)) (property \"Value\" \"Part\" (at 0 0 0)) (symbol \"Part_1_1\" (pin passive line (at 0 0 0) (length 0) (number \"1\"))) (symbol \"Part_2_1\" (pin passive line (at 0 0 0) (length 0) (number \"2\"))))").unwrap();
        let mut source = Document::new();
        source.add_symbol("Logic:Part", &def, 10.16, 10.16, 1);
        source.add_symbol("Logic:Part", &def, 20.32, 10.16, 2);
        let text = copy(&source, None).unwrap();
        let mut dest = Document::new();
        let mut changed = def.clone();
        changed.set_child(Node::list("future_metadata", vec![Node::atom("different")]));
        dest.add_symbol("Logic:Part", &changed, 50.8, 50.8, 1);
        let original = dest.embedded("Logic:Part").unwrap().clone();
        let ids = paste(&mut dest, &text, Some((71.12, 71.12))).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(dest.embedded("Logic:Part"), Some(&original));
        let symbols: Vec<_> = dest.root.children("symbol").collect();
        assert_eq!(property(symbols[1], "Reference"), "U2");
        assert_eq!(property(symbols[2], "Reference"), "U2");
        assert_eq!(
            symbols[1].child("lib_id").unwrap().value(1),
            "Logic:Part_jenga2"
        );
        assert_eq!(crate::scene::Scene::from_document(&dest).pins.len(), 3);
        let again = paste(&mut dest, &text, None).unwrap();
        assert_eq!(again.len(), 2);
        assert!(
            dest.root
                .children("symbol")
                .skip(3)
                .all(|n| property(n, "Reference") == "U3")
        );
    }
    #[test]
    fn wire_fragment_roundtrip_and_atomic_rejection() {
        let mut d = Document::new();
        d.add_wire((10.16, 10.16), (20.32, 10.16));
        let text = copy(&d, None).unwrap();
        let before = d.root.clone();
        assert!(paste(&mut d, "(sheet (at 0 0))", None).is_err());
        assert_eq!(d.root, before);
        let ids = paste(&mut d, &text, Some((30.48, 30.48))).unwrap();
        assert_eq!(ids.len(), 1);
        assert_ne!(
            d.root
                .children("wire")
                .next()
                .unwrap()
                .child("uuid")
                .unwrap()
                .value(1),
            ids[0]
        );
        assert_eq!(
            points(d.root.children("wire").nth(1).unwrap(), "pts"),
            vec![(30.48, 30.48), (40.64, 30.48)]
        );
    }
}
