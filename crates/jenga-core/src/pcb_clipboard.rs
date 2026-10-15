//! Native board fragments with fresh identities and explicit named-net joins.
use crate::{
    pcb::{self, Board},
    sexpr::{Node, parse},
};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
pub fn copy(b: &Board, ids: Option<&[String]>) -> Result<String> {
    let mut root = b.root.clone();
    root.items_mut().retain(|n| {
        !matches!(
            n.name(),
            "footprint"
                | "segment"
                | "arc"
                | "via"
                | "zone"
                | "gr_text"
                | "gr_line"
                | "gr_arc"
                | "gr_rect"
                | "gr_circle"
                | "gr_poly"
                | "group"
        ) || ids.is_none_or(|ids| ids.iter().any(|id| id == pcb::id(n)))
    });
    let mut used = HashSet::new();
    fn codes(n: &Node, out: &mut HashSet<i64>) {
        if n.name() == "net" {
            out.insert(n.number(1) as i64);
        } else {
            for c in n.items() {
                codes(c, out);
            }
        }
    }
    for n in root
        .items()
        .iter()
        .filter(|n| matches!(n.name(), "footprint" | "segment" | "arc" | "via" | "zone"))
    {
        codes(n, &mut used);
    }
    root.items_mut()
        .retain(|n| n.name() != "net" || used.contains(&(n.number(1) as i64)));
    Ok(root.document_text())
}
pub fn paste(
    b: &mut Board,
    text: &str,
    at: Option<(f64, f64)>,
    allow_merges: bool,
) -> Result<Value> {
    ensure!(
        text.len() <= 32 * 1024 * 1024,
        "PCB clipboard exceeds 32 MiB"
    );
    let root = parse(text)?;
    ensure!(
        root.name() == "kicad_pcb",
        "Expected native KiCad PCB clipboard data"
    );
    let mut candidate = b.snapshot();
    let mut imported = b.snapshot();
    imported.root = root.clone();
    imported.extra_nets.clear();
    let source = imported.nets();
    let existing = b.nets();
    let joins: Vec<_> = source
        .values()
        .filter(|n| !n.is_empty() && existing.values().any(|x| x == *n))
        .cloned()
        .collect();
    ensure!(
        allow_merges || joins.is_empty(),
        "Pasting would join existing named nets {:?}; inspect and explicitly allow_named_net_merges to join them",
        joins
    );
    let mut nets = BTreeMap::from([(0, 0)]);
    for (code, name) in source {
        if code != 0 {
            nets.insert(code, candidate.ensure_net(&name));
        }
    }
    let supported = [
        "footprint",
        "segment",
        "arc",
        "via",
        "zone",
        "gr_text",
        "gr_line",
        "gr_arc",
        "gr_rect",
        "gr_circle",
        "gr_poly",
        "group",
    ];
    let mut objects: Vec<_> = root
        .items()
        .iter()
        .filter(|n| supported.contains(&n.name()))
        .cloned()
        .collect();
    let unsupported: Vec<_> = root
        .items()
        .iter()
        .filter(|n| n.child("uuid").is_some() && !supported.contains(&n.name()))
        .map(|n| n.name())
        .collect();
    ensure!(
        unsupported.is_empty(),
        "Unsupported PCB clipboard objects {:?}; import the complete native board to preserve them",
        unsupported
    );
    ensure!(!objects.is_empty(), "No supported PCB objects in clipboard");
    let mut ids = BTreeMap::new();
    fn collect(n: &Node, m: &mut BTreeMap<String, String>) {
        if n.name() == "uuid" || n.name() == "tstamp" {
            m.entry(n.value(1).into())
                .or_insert_with(|| uuid::Uuid::new_v4().to_string());
        }
        for c in n.items() {
            collect(c, m);
        }
    }
    for n in &objects {
        collect(n, &mut ids);
    }
    fn rewrite(
        n: &mut Node,
        ids: &BTreeMap<String, String>,
        nets: &BTreeMap<i64, i64>,
        names: &BTreeMap<i64, String>,
        named: bool,
    ) -> Result<()> {
        if n.name() == "net" {
            let code = if matches!(n.items().get(1), Some(Node::String(_))) {
                pcb::named_net_code(n.value(1))
            } else {
                n.number(1) as i64
            };
            ensure!(
                nets.contains_key(&code),
                "Clipboard net {code} has no definition; export/copy with net names included"
            );
            *n = pcb::node(
                "net",
                if named {
                    vec![Node::string(
                        names.get(&nets[&code]).cloned().unwrap_or_default(),
                    )]
                } else {
                    vec![
                        Node::atom(nets[&code]),
                        Node::string(names.get(&nets[&code]).cloned().unwrap_or_default()),
                    ]
                },
            );
        }
        if let Node::List(items) = n {
            for c in items {
                match c {
                    Node::List(_) => rewrite(c, ids, nets, names, named)?,
                    _ => {
                        let value = c.text();
                        if let Some(new) = ids.get(value) {
                            *c = Node::string(new);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    // A footprint's local geometry is not translated; only its board origin is.
    let origins: Vec<_> = objects
        .iter()
        .filter_map(|n| n.child("at").or_else(|| n.child("start")))
        .map(|n| (n.number(1), n.number(2)))
        .collect();
    let origin = origins.first().copied().unwrap_or((0., 0.));
    let delta = at
        .map(|p| (p.0 - origin.0, p.1 - origin.1))
        .unwrap_or((0., 0.));
    let mut refs: HashSet<_> = b
        .root
        .children("footprint")
        .map(|n| pcb::field(n, "Reference").to_owned())
        .collect();
    let mut added = vec![];
    let mut renamed = vec![];
    for n in &mut objects {
        rewrite(n, &ids, &nets, &candidate.nets(), candidate.named_nets())?;
        if n.name() == "footprint" {
            n.items_mut()
                .retain(|n| !matches!(n.name(), "path" | "sheetname" | "sheetfile"));
            let old = pcb::field(n, "Reference").to_owned();
            if refs.contains(&old) {
                let prefix: String = old.chars().take_while(|c| !c.is_ascii_digit()).collect();
                let mut i = 1;
                while refs.contains(&format!("{prefix}{i}")) {
                    i += 1;
                }
                let new = format!("{prefix}{i}");
                pcb::set_field(n, "Reference", &new);
                renamed.push(json!({"old":old,"new":new}));
            }
            refs.insert(pcb::field(n, "Reference").into());
        }
        for key in if n.name() == "footprint" {
            &["at"][..]
        } else {
            &["at", "start", "end", "mid", "center"][..]
        } {
            if let Some(p) = n.child_mut(key) {
                let x = p.number(1) + delta.0;
                let y = p.number(2) + delta.1;
                p.items_mut()[1] = Node::atom(x);
                p.items_mut()[2] = Node::atom(y);
            }
        }
        if n.name() != "footprint" {
            fn translate_pts(n: &mut Node, d: (f64, f64)) {
                if n.name() == "xy" {
                    let x = n.number(1) + d.0;
                    let y = n.number(2) + d.1;
                    n.items_mut()[1] = Node::atom(x);
                    n.items_mut()[2] = Node::atom(y);
                } else if let Node::List(items) = n {
                    for child in items {
                        translate_pts(child, d);
                    }
                }
            }
            translate_pts(n, delta);
        }
        added.push(pcb::id(n).to_owned());
    }
    candidate.root.items_mut().extend(objects);
    candidate.dirty = true;
    candidate.undo = std::mem::take(&mut b.undo);
    *b = candidate;
    Ok(
        json!({"added_uuids":added,"renamed_references":renamed,"joined_nets":joins,"dirty":true,"saved":false}),
    )
}
