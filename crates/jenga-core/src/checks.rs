//! Basic ERC for a single sheet. Crossings connect only at endpoints or junctions.
use crate::{
    document::{Document, position, property},
    scene::{Point, Scene, points},
};
use std::collections::HashMap;
#[derive(Clone, Debug)]
pub struct Issue {
    pub severity: &'static str,
    pub message: String,
    pub object: Option<usize>,
}
pub fn on_segment(p: Point, a: Point, b: Point) -> bool {
    let cross = (p.0 - a.0) * (b.1 - a.1) - (p.1 - a.1) * (b.0 - a.0);
    cross.abs() < 1e-5
        && p.0 >= a.0.min(b.0) - 1e-5
        && p.0 <= a.0.max(b.0) + 1e-5
        && p.1 >= a.1.min(b.1) - 1e-5
        && p.1 <= a.1.max(b.1) + 1e-5
}
fn near(a: Point, b: Point) -> bool {
    (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5
}
pub fn run(doc: &Document) -> Vec<Issue> {
    let scene = Scene::from_document(doc);
    let mut issues = vec![];
    let mut refs = HashMap::new();
    for (index, n) in doc
        .root
        .items()
        .iter()
        .enumerate()
        .filter(|(_, n)| n.name() == "symbol")
    {
        let reference = property(n, "Reference");
        let unit = n.child("unit").map(|n| n.value(1)).unwrap_or("1");
        if reference.is_empty() || reference.ends_with('?') {
            issues.push(Issue {
                severity: "Warning",
                message: "Symbol needs annotation".into(),
                object: Some(index),
            });
        }
        if refs.insert((reference, unit), index).is_some() {
            issues.push(Issue {
                severity: "Error",
                message: format!("Duplicate reference/unit: {reference} / {unit}"),
                object: Some(index),
            });
        }
    }
    let wires: Vec<_> = doc
        .root
        .children("wire")
        .filter_map(|n| {
            let p = points(n, "pts");
            if p.len() == 2 {
                Some((p[0], p[1]))
            } else {
                None
            }
        })
        .collect();
    let nc: Vec<_> = doc
        .root
        .children("no_connect")
        .map(|n| {
            let (x, y, _) = position(n);
            (x, y)
        })
        .collect();
    let junctions: Vec<_> = doc
        .root
        .children("junction")
        .map(|n| {
            let (x, y, _) = position(n);
            (x, y)
        })
        .collect();
    // Nodes are wires followed by pins. Join endpoint T connections, never bare crossings.
    let len = wires.len() + scene.pins.len();
    let mut parents: Vec<_> = (0..len).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    fn union(p: &mut [usize], a: usize, b: usize) {
        let a = root(p, a);
        let b = root(p, b);
        p[b] = a;
    }
    for (i, &(a, b)) in wires.iter().enumerate() {
        for (j, &(c, d)) in wires.iter().enumerate().take(i) {
            if on_segment(a, c, d)
                || on_segment(b, c, d)
                || on_segment(c, a, b)
                || on_segment(d, a, b)
                || junctions
                    .iter()
                    .any(|p| on_segment(*p, a, b) && on_segment(*p, c, d))
            {
                union(&mut parents, i, j);
            }
        }
    }
    for (i, pin) in scene.pins.iter().enumerate() {
        let pi = wires.len() + i;
        let mut connected = false;
        for (j, &(a, b)) in wires.iter().enumerate() {
            if on_segment(pin.at, a, b) {
                union(&mut parents, pi, j);
                connected = true;
            }
        }
        for (j, other) in scene.pins.iter().enumerate().take(i) {
            if near(pin.at, other.at) {
                union(&mut parents, pi, wires.len() + j);
                connected = true;
            }
        }
        connected |= scene
            .pins
            .iter()
            .enumerate()
            .any(|(j, p)| j != i && near(p.at, pin.at));
        let marked = nc.iter().any(|p| near(*p, pin.at));
        if marked && connected {
            issues.push(Issue {
                severity: "Error",
                message: format!(
                    "{} pin {} has both a connection and no-connect marker",
                    pin.reference, pin.number
                ),
                object: Some(pin.object),
            });
        }
        if !connected && !marked && pin.kind != "no_connect" {
            issues.push(Issue {
                severity: "Warning",
                message: format!(
                    "Unconnected pin: {}.{} ({})",
                    pin.reference, pin.number, pin.kind
                ),
                object: Some(pin.object),
            });
        }
    }
    let mut labels: HashMap<String, usize> = HashMap::new();
    for n in doc
        .root
        .items()
        .iter()
        .filter(|n| matches!(n.name(), "label" | "global_label" | "hierarchical_label"))
    {
        let (x, y, _) = position(n);
        let candidates: Vec<_> = wires
            .iter()
            .enumerate()
            .filter(|(_, (a, b))| on_segment((x, y), *a, *b))
            .map(|(i, _)| i)
            .chain(
                scene
                    .pins
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| near(p.at, (x, y)))
                    .map(|(i, _)| wires.len() + i),
            )
            .collect();
        for i in candidates {
            if let Some(previous) = labels.insert(n.value(1).into(), i) {
                union(&mut parents, i, previous);
            }
        }
    }
    // Power symbols provide implicit named net labels at their pins.
    for (i, pin) in scene.pins.iter().enumerate() {
        let n = &doc.root.items()[pin.object];
        if doc
            .instance_definition(n)
            .is_some_and(|s| s.child("power").is_some())
        {
            let name = property(n, "Value").to_owned();
            let pi = wires.len() + i;
            if let Some(previous) = labels.insert(name, pi) {
                union(&mut parents, pi, previous);
            }
        }
    }
    let mut nets: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..scene.pins.len() {
        nets.entry(root(&mut parents, wires.len() + i))
            .or_default()
            .push(i);
    }
    for pins in nets.values() {
        let drivers = pins
            .iter()
            .filter(|i| scene.pins[**i].kind == "power_out")
            .count();
        let outputs = pins
            .iter()
            .filter(|i| scene.pins[**i].kind == "output")
            .count();
        if outputs > 1 {
            issues.push(Issue {
                severity: "Error",
                message: "Multiple output pins drive the same net".into(),
                object: Some(scene.pins[pins[0]].object),
            });
        }
        for &i in pins {
            let pin = &scene.pins[i];
            if pin.kind == "power_in" && drivers == 0 && !nc.iter().any(|p| near(*p, pin.at)) {
                issues.push(Issue {
                    severity: "Warning",
                    message: format!(
                        "{} pin {}: power input has no power-output driver (check supply/PWR_FLAG)",
                        pin.reference, pin.number
                    ),
                    object: Some(pin.object),
                });
            }
        }
    }
    for missing in scene.missing {
        issues.push(Issue {
            severity: "Error",
            message: format!("Missing embedded symbol: {missing}"),
            object: None,
        });
    }
    issues
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr::parse;
    #[test]
    fn crossing_without_junction_does_not_short_outputs() {
        let def=parse("(symbol \"X\" (property \"Reference\" \"U\" (at 0 1 0)) (symbol \"X_1_1\" (pin output line (at 0 0 0) (length 2) (name \"OUT\") (number \"1\"))))").unwrap();
        let mut d = Document::new();
        d.add_symbol("test:X", &def, 0., 5., 1);
        d.add_symbol("test:X", &def, 5., 0., 1);
        d.add_wire((0., 5.), (10., 5.));
        d.add_wire((5., 0.), (5., 10.));
        assert!(
            !run(&d)
                .iter()
                .any(|i| i.message.contains("Multiple output"))
        );
        d.add_point("junction", 5., 5.);
        assert!(
            run(&d)
                .iter()
                .any(|i| i.message.contains("Multiple output"))
        );
    }
}
