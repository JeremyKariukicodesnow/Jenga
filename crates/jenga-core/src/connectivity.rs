//! Inspect actual electrical groups, including explicit named nets, not visual proximity.
use crate::{
    checks::on_segment,
    document::{Document, position, property},
    scene::{Scene, points},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
pub fn inspect(doc: &Document) -> Value {
    let scene = Scene::from_document(doc);
    let wires: Vec<_> = doc
        .root
        .children("wire")
        .filter_map(|n| {
            let p = points(n, "pts");
            (p.len() == 2).then(|| (p[0], p[1]))
        })
        .collect();
    let mut parent: Vec<usize> = (0..wires.len() + scene.pins.len()).collect();
    fn root(p: &mut [usize], i: usize) -> usize {
        if p[i] != i {
            p[i] = root(p, p[i]);
        }
        p[i]
    }
    fn join(p: &mut [usize], a: usize, b: usize) {
        let a = root(p, a);
        let b = root(p, b);
        p[b] = a;
    }
    let junctions: Vec<_> = doc
        .root
        .children("junction")
        .map(|n| {
            let (x, y, _) = position(n);
            (x, y)
        })
        .collect();
    for (i, (a, b)) in wires.iter().enumerate() {
        for (j, (c, d)) in wires.iter().enumerate().take(i) {
            if on_segment(*a, *c, *d)
                || on_segment(*b, *c, *d)
                || on_segment(*c, *a, *b)
                || on_segment(*d, *a, *b)
                || junctions
                    .iter()
                    .any(|p| on_segment(*p, *a, *b) && on_segment(*p, *c, *d))
            {
                join(&mut parent, i, j);
            }
        }
    }
    for (i, pin) in scene.pins.iter().enumerate() {
        let pi = wires.len() + i;
        for (j, (a, b)) in wires.iter().enumerate() {
            if on_segment(pin.at, *a, *b) {
                join(&mut parent, pi, j);
            }
        }
        for (j, other) in scene.pins.iter().enumerate().take(i) {
            if pin.at == other.at
                || (pin.reference == other.reference && pin.number == other.number)
            {
                join(&mut parent, pi, wires.len() + j);
            }
        }
    }
    let mut names: HashMap<String, usize> = HashMap::new();
    let mut named = vec![];
    for n in doc
        .root
        .items()
        .iter()
        .filter(|n| matches!(n.name(), "label" | "global_label" | "hierarchical_label"))
    {
        let (x, y, _) = position(n);
        let at = (x, y);
        let name = n.value(1).to_owned();
        for i in wires
            .iter()
            .enumerate()
            .filter(|(_, (a, b))| on_segment(at, *a, *b))
            .map(|(i, _)| i)
            .chain(
                scene
                    .pins
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.at == at)
                    .map(|(i, _)| wires.len() + i),
            )
        {
            if let Some(other) = names.insert(name.clone(), i) {
                join(&mut parent, i, other);
            }
            named.push((name.clone(), i));
        }
    }
    for (i, pin) in scene.pins.iter().enumerate() {
        let n = &doc.root.items()[pin.object];
        let hidden = pin.hidden && matches!(pin.kind.as_str(), "power_in" | "power_out");
        if hidden
            || pin.kind == "power_in"
                && doc
                    .instance_definition(n)
                    .is_some_and(|s| s.child("power").is_some())
        {
            let name = if hidden {
                pin.name.clone()
            } else {
                property(n, "Value").to_owned()
            };
            let pi = wires.len() + i;
            if let Some(other) = names.insert(name.clone(), pi) {
                join(&mut parent, pi, other);
            }
            named.push((name, pi));
        }
    }
    let mut groups: BTreeMap<usize, Value> = BTreeMap::new();
    for (i, pin) in scene.pins.iter().enumerate() {
        let key = root(&mut parent, wires.len() + i);
        let group = groups
            .entry(key)
            .or_insert(json!({"names":[],"pins":[],"wire_segments":0}));
        group["pins"].as_array_mut().unwrap().push(json!({"reference":pin.reference,"number":pin.number,"electrical_type":pin.kind,"at_mm":pin.at,"uuid":doc.root.items()[pin.object].child("uuid").map(|n|n.value(1))}));
    }
    for i in 0..wires.len() {
        let key = root(&mut parent, i);
        let group = groups
            .entry(key)
            .or_insert(json!({"names":[],"pins":[],"wire_segments":0}));
        group["wire_segments"] = json!(group["wire_segments"].as_u64().unwrap() + 1);
    }
    for (name, i) in named {
        let key = root(&mut parent, i);
        if let Some(group) = groups.get_mut(&key) {
            let list = group["names"].as_array_mut().unwrap();
            if !list.contains(&json!(name)) {
                list.push(json!(name));
            }
        }
    }
    json!({"scope":"single_sheet","revision":doc.root_uuid(),"nets":groups.into_values().collect::<Vec<_>>(),"note":"Matching names join electrically. Bare wire crossings do not join. ERC is a separate check; inspect these pin memberships to verify intended wiring."})
}
