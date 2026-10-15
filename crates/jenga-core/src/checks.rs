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
#[derive(Clone, Debug, serde::Serialize)]
pub struct Finding {
    pub severity: String,
    pub message: String,
    pub object: Option<usize>,
    pub uuid: Option<String>,
    pub location: Option<Point>,
    pub suggestion: String,
}
pub fn describe(doc: &Document, issue: &Issue) -> Finding {
    let scene = Scene::from_document(doc);
    describe_with_scene(doc, issue, &scene)
}
pub fn findings(doc: &Document) -> Vec<Finding> {
    let scene = Scene::from_document(doc);
    run(doc)
        .iter()
        .map(|i| describe_with_scene(doc, i, &scene))
        .collect()
}
pub fn describe_with_scene(doc: &Document, issue: &Issue, scene: &Scene) -> Finding {
    let location = scene
        .pins
        .iter()
        .find(|p| {
            Some(p.object) == issue.object
                && (issue
                    .message
                    .contains(&format!("{}.{}", p.reference, p.number))
                    || issue
                        .message
                        .contains(&format!("{} pin {}", p.reference, p.number)))
        })
        .map(|p| p.at)
        .or_else(|| {
            issue
                .object
                .and_then(|i| scene.object_bounds(i))
                .map(|(a, b)| ((a.0 + b.0) / 2., (a.1 + b.1) / 2.))
        })
        .or_else(|| {
            issue
                .object
                .and_then(|i| doc.root.items().get(i))
                .filter(|n| n.child("at").is_some())
                .map(|n| {
                    let (x, y, _) = position(n);
                    (x, y)
                })
        });
    let m = &issue.message;
    let suggestion = if m.contains("both a connection") {
        "Remove the no-connect marker if this pin is wired, or remove its connection if intentionally unused."
    } else if m.contains("Unconnected pin") {
        "Connect this pin to the intended net. If deliberately unused, place a no-connect marker exactly on the pin."
    } else if m.contains("power-output driver") {
        "Verify the supply is connected. Use a genuine power-output pin or PWR_FLAG only when the net is supplied externally."
    } else if m.contains("Multiple output") {
        "Separate the output drivers or check their electrical pin types. Do not short ordinary signal outputs or independent supplies."
    } else if m.contains("Duplicate reference") {
        "Assign a unique reference to this component; keep the same reference only for distinct units of one part."
    } else if m.contains("annotation") {
        "Edit the Reference field and assign a unique designator such as R1 or U1."
    } else if m.contains("Missing embedded") {
        "Restore the embedded symbol definition from its original library before saving or exporting."
    } else if m.contains("multiple names") {
        "Check labels on this net and choose one intended net name."
    } else if m.contains("no driver") {
        "Connect an appropriate output or passive source to this input net."
    } else if m.contains("internally as no-connect") {
        "Remove the wire from this pin. The library declares that it must remain unused."
    } else {
        "Move the marker or label onto the exact wire or pin position using grid snapping."
    };
    Finding {
        severity: issue.severity.into(),
        message: issue.message.clone(),
        object: issue.object,
        uuid: issue
            .object
            .and_then(|i| doc.root.items().get(i)?.child("uuid"))
            .map(|n| n.value(1).into()),
        location,
        suggestion: suggestion.into(),
    }
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
            if near(pin.at, other.at)
                || (pin.reference == other.reference && pin.number == other.number)
            {
                union(&mut parents, pi, wires.len() + j);
                connected = true;
            }
        }
        connected |= scene
            .pins
            .iter()
            .enumerate()
            .any(|(j, p)| j != i && near(p.at, pin.at));
        connected |= doc
            .root
            .items()
            .iter()
            .filter(|n| matches!(n.name(), "label" | "global_label" | "hierarchical_label"))
            .any(|n| {
                let (x, y, _) = position(n);
                near((x, y), pin.at)
            });
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
    let mut named_nodes: Vec<(String, usize)> = vec![];
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
            named_nodes.push((n.value(1).into(), i));
            if let Some(previous) = labels.insert(n.value(1).into(), i) {
                union(&mut parents, i, previous);
            }
        }
    }
    // Power symbols provide implicit named net labels at their pins.
    for (i, pin) in scene.pins.iter().enumerate() {
        let n = &doc.root.items()[pin.object];
        if (pin.hidden && matches!(pin.kind.as_str(), "power_in" | "power_out"))
            || doc
                .instance_definition(n)
                .is_some_and(|s| s.child("power").is_some())
        {
            let name = if pin.hidden && matches!(pin.kind.as_str(), "power_in" | "power_out") {
                pin.name.clone()
            } else {
                property(n, "Value").to_owned()
            };
            let pi = wires.len() + i;
            named_nodes.push((name.clone(), pi));
            if let Some(previous) = labels.insert(name, pi) {
                union(&mut parents, pi, previous);
            }
        }
    }
    let mut nets: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut names: HashMap<usize, Vec<String>> = HashMap::new();
    for (name, index) in named_nodes {
        let list = names.entry(root(&mut parents, index)).or_default();
        if !list.contains(&name) {
            list.push(name);
        }
    }
    for (net, list) in names.iter().filter(|(_, n)| n.len() > 1) {
        let object = scene
            .pins
            .iter()
            .enumerate()
            .find(|(i, _)| root(&mut parents, wires.len() + i) == *net)
            .map(|(_, p)| p.object)
            .or_else(|| {
                doc.root
                    .items()
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.name() == "wire")
                    .enumerate()
                    .find(|(wire, _)| root(&mut parents, *wire) == *net)
                    .map(|(_, (i, _))| i)
            });
        issues.push(Issue {
            severity: "Warning",
            message: format!("Net has multiple names: {}", list.join(", ")),
            object,
        });
    }
    // Resolve named/hidden power connections before reporting disconnected pins.
    issues.retain(|issue| {
        if !issue.message.starts_with("Unconnected pin:") {
            return true;
        }
        let Some((i, _)) = scene.pins.iter().enumerate().find(|(_, p)| {
            Some(p.object) == issue.object
                && issue
                    .message
                    .starts_with(&format!("Unconnected pin: {}.{} (", p.reference, p.number))
        }) else {
            return true;
        };
        let net = root(&mut parents, wires.len() + i);
        !scene
            .pins
            .iter()
            .enumerate()
            .any(|(j, _)| j != i && root(&mut parents, wires.len() + j) == net)
    });
    for i in 0..scene.pins.len() {
        nets.entry(root(&mut parents, wires.len() + i))
            .or_default()
            .push(i);
    }
    for pins in nets.values() {
        let mut physical = std::collections::HashSet::new();
        let pins: Vec<_> = pins
            .iter()
            .copied()
            .filter(|i| {
                physical.insert((
                    scene.pins[*i].reference.clone(),
                    scene.pins[*i].number.clone(),
                ))
            })
            .collect();
        let drivers = pins
            .iter()
            .filter(|i| scene.pins[**i].kind == "power_out")
            .count();
        let outputs = pins
            .iter()
            .filter(|i| scene.pins[**i].kind == "output")
            .count();
        if outputs > 1 || drivers > 1 || (outputs > 0 && drivers > 0) {
            issues.push(Issue {
                severity: "Error",
                message: "Multiple output pins drive the same net (signal or power)".into(),
                object: Some(scene.pins[pins[0]].object),
            });
        }
        for &i in &pins {
            let pin = &scene.pins[i];
            if pin.kind == "no_connect"
                && (pins.len() > 1 || wires.iter().any(|(a, b)| on_segment(pin.at, *a, *b)))
            {
                issues.push(Issue {
                    severity: "Error",
                    message: format!(
                        "{}.{}: a pin marked internally as no-connect is connected",
                        pin.reference, pin.number
                    ),
                    object: Some(pin.object),
                });
            }
            if pin.kind == "input"
                && pins.len() > 1
                && !pins.iter().any(|i| {
                    matches!(
                        scene.pins[*i].kind.as_str(),
                        "output"
                            | "bidirectional"
                            | "tri_state"
                            | "open_collector"
                            | "open_emitter"
                            | "passive"
                            | "power_out"
                    )
                })
            {
                issues.push(Issue {
                    severity: "Warning",
                    message: format!("{}.{}: input net has no driver", pin.reference, pin.number),
                    object: Some(pin.object),
                });
            }
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
            object: doc.root.items().iter().position(|n| {
                n.name() == "symbol" && n.child("lib_id").is_some_and(|n| n.value(1) == missing)
            }),
        });
    }
    for (index, n) in doc.root.items().iter().enumerate() {
        if matches!(
            n.name(),
            "label" | "global_label" | "hierarchical_label" | "junction" | "no_connect"
        ) {
            let (x, y, _) = position(n);
            let at = (x, y);
            let pin = scene.pins.iter().any(|p| near(p.at, at));
            let wire = wires.iter().any(|(a, b)| on_segment(at, *a, *b));
            if (n.name() == "no_connect" && !pin) || (n.name() != "no_connect" && !pin && !wire) {
                issues.push(Issue {
                    severity: "Warning",
                    message: format!(
                        "{} is not attached to {}",
                        n.name().replace('_', " "),
                        if n.name() == "no_connect" {
                            "a pin"
                        } else {
                            "a wire or pin"
                        }
                    ),
                    object: Some(index),
                });
            }
        }
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
