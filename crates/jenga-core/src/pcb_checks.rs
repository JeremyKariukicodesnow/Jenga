//! Deterministic copper connectivity and conservative board checks.
use crate::{
    pcb::{self, Board, Pad},
    scene::Point,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
const EPS: f64 = 1e-6;
pub fn point_segment(p: Point, a: Point, b: Point) -> f64 {
    let d = (b.0 - a.0, b.1 - a.1);
    let n = d.0 * d.0 + d.1 * d.1;
    let t = if n < EPS * EPS {
        0.
    } else {
        ((p.0 - a.0) * d.0 + (p.1 - a.1) * d.1) / n
    }
    .clamp(0., 1.);
    (p.0 - a.0 - t * d.0).hypot(p.1 - a.1 - t * d.1)
}
pub fn segment_distance(a: Point, b: Point, c: Point, d: Point) -> f64 {
    let cross =
        |p: Point, q: Point, r: Point| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let ab1 = cross(a, b, c);
    let ab2 = cross(a, b, d);
    let cd1 = cross(c, d, a);
    let cd2 = cross(c, d, b);
    if ab1 * ab2 < -EPS * EPS && cd1 * cd2 < -EPS * EPS {
        return 0.;
    }
    point_segment(a, c, d)
        .min(point_segment(b, c, d))
        .min(point_segment(c, a, b))
        .min(point_segment(d, a, b))
}
pub fn inside_polygon(p: Point, poly: &[Point]) -> bool {
    let mut inside = false;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if point_segment(p, a, b) < EPS {
            return true;
        }
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
    }
    inside
}
#[derive(Clone)]
pub enum Geometry {
    Stroke(Point, Point, f64),
    Polygon(Vec<Point>),
}
impl Geometry {
    pub fn center(&self) -> Point {
        match self {
            Self::Stroke(a, b, _) => ((a.0 + b.0) / 2., (a.1 + b.1) / 2.),
            Self::Polygon(p) => {
                let (a, b) = crate::scene::bounds(p);
                ((a.0 + b.0) / 2., (a.1 + b.1) / 2.)
            }
        }
    }
    pub fn distance(&self, other: &Self) -> f64 {
        match (self, other) {
            (Self::Stroke(a, b, r), Self::Stroke(c, d, s)) => {
                segment_distance(*a, *b, *c, *d) - r - s
            }
            (Self::Stroke(a, b, r), Self::Polygon(poly))
            | (Self::Polygon(poly), Self::Stroke(a, b, r)) => {
                if inside_polygon(*a, poly) || inside_polygon(*b, poly) {
                    return -*r;
                }
                let mut distance = f64::INFINITY;
                for i in 0..poly.len() {
                    distance = distance.min(segment_distance(
                        *a,
                        *b,
                        poly[i],
                        poly[(i + 1) % poly.len()],
                    ));
                }
                distance - r
            }
            (Self::Polygon(a), Self::Polygon(b)) => {
                if a.is_empty() || b.is_empty() {
                    return f64::INFINITY;
                }
                if inside_polygon(a[0], b) || inside_polygon(b[0], a) {
                    return 0.;
                }
                let mut distance = f64::INFINITY;
                for i in 0..a.len() {
                    for j in 0..b.len() {
                        distance = distance.min(segment_distance(
                            a[i],
                            a[(i + 1) % a.len()],
                            b[j],
                            b[(j + 1) % b.len()],
                        ));
                    }
                }
                distance
            }
        }
    }
}
pub fn pad_geometry(p: &Pad) -> Geometry {
    if p.shape == "circle" {
        return Geometry::Stroke(p.at, p.at, p.size.0.min(p.size.1) / 2.);
    }
    if p.shape == "oval" {
        let r = p.size.0.min(p.size.1) / 2.;
        let half = (p.size.0.max(p.size.1) - 2. * r) / 2.;
        let (a, b) = if p.size.0 >= p.size.1 {
            ((-half, 0.), (half, 0.))
        } else {
            ((0., -half), (0., half))
        };
        return Geometry::Stroke(
            pcb::transform(a, p.at, p.angle),
            pcb::transform(b, p.at, p.angle),
            r,
        );
    }
    let w = p.size.0 / 2.;
    let h = p.size.1 / 2.;
    Geometry::Polygon(
        [(-w, -h), (w, -h), (w, h), (-w, h)]
            .iter()
            .map(|p0| pcb::transform(*p0, p.at, p.angle))
            .collect(),
    )
}
#[derive(Clone)]
pub struct Copper {
    pub uuid: String,
    pub kind: String,
    pub reference: String,
    pub pad: String,
    pub net: i64,
    pub layers: Vec<String>,
    pub geometry: Geometry,
    pub pad_index: Option<usize>,
}
impl Copper {
    pub fn shares_layer(&self, other: &Self) -> bool {
        self.layers
            .iter()
            .any(|l| l == "*.Cu" || other.layers.iter().any(|m| m == "*.Cu" || m == l))
    }
}
pub fn copper(board: &Board) -> Vec<Copper> {
    let mut c = vec![];
    for (i, p) in board.pads().iter().enumerate() {
        let mechanical = p.kind == "np_thru_hole";
        c.push(Copper {
            uuid: p.uuid.clone(),
            kind: if mechanical {
                "hole".into()
            } else {
                "pad".into()
            },
            reference: p.reference.clone(),
            pad: p.number.clone(),
            net: if mechanical { -1 } else { p.net },
            layers: if mechanical {
                vec!["*.Cu".into()]
            } else {
                p.layers
                    .iter()
                    .filter(|l| l.ends_with(".Cu"))
                    .cloned()
                    .collect()
            },
            geometry: if mechanical {
                Geometry::Stroke(p.at, p.at, p.drill / 2.)
            } else {
                pad_geometry(p)
            },
            pad_index: Some(i),
        });
    }
    for n in board.root.items() {
        let net = pcb::net_code(n);
        let uuid = pcb::id(n).to_owned();
        let layers = if n.name() == "via" {
            if n.items().iter().any(|n| n.text() == "blind") {
                n.child("layers")
                    .map(|n| {
                        n.items()
                            .iter()
                            .skip(1)
                            .map(|n| n.text().to_owned())
                            .collect()
                    })
                    .unwrap_or_else(|| vec!["*.Cu".into()])
            } else {
                board.copper_layers()
            }
        } else {
            vec![pcb::layer(n).to_owned()]
        };
        let mut push = |geometry| {
            c.push(Copper {
                uuid: uuid.clone(),
                kind: n.name().into(),
                reference: String::new(),
                pad: String::new(),
                net,
                layers: layers.clone(),
                geometry,
                pad_index: None,
            })
        };
        match n.name() {
            "segment" => push(Geometry::Stroke(
                pcb::point(n, "start"),
                pcb::point(n, "end"),
                n.child("width").map(|n| n.number(1) / 2.).unwrap_or(0.125),
            )),
            "arc" => {
                let p = pcb::arc_points(n);
                let width = n.child("width").map(|n| n.number(1) / 2.).unwrap_or(0.125);
                for w in p.windows(2) {
                    push(Geometry::Stroke(w[0], w[1], width));
                }
            }
            "via" => push(Geometry::Stroke(
                pcb::point(n, "at"),
                pcb::point(n, "at"),
                n.child("size").map(|n| n.number(1) / 2.).unwrap_or(0.3),
            )),
            "zone" => {
                if n.child("keepout").is_none() {
                    for poly in n.children("filled_polygon") {
                        let p = pcb::pts(poly);
                        if p.len() >= 3 {
                            push(Geometry::Polygon(p));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    c
}
pub fn partition(board: &Board) -> Vec<usize> {
    let c = copper(board);
    let mut parent: Vec<_> = (0..c.len()).collect();
    fn root(p: &mut [usize], i: usize) -> usize {
        if p[i] != i {
            p[i] = root(p, p[i]);
        }
        p[i]
    }
    for i in 0..c.len() {
        for j in 0..i {
            if c[i].net > 0
                && c[i].net == c[j].net
                && c[i].shares_layer(&c[j])
                && c[i].geometry.distance(&c[j].geometry) <= EPS
            {
                let a = root(&mut parent, i);
                let b = root(&mut parent, j);
                parent[b] = a;
            }
        }
    }
    for i in 0..parent.len() {
        parent[i] = root(&mut parent, i);
    }
    parent
}
pub fn connected(board: &Board, a: &Pad, b: &Pad) -> bool {
    if a.net <= 0 || a.net != b.net {
        return false;
    }
    let c = copper(board);
    let p = partition(board);
    let find = |pad: &Pad| {
        c.iter().position(|n| {
            n.kind == "pad"
                && n.reference == pad.reference
                && n.pad == pad.number
                && n.geometry.center() == pad_geometry(pad).center()
        })
    };
    find(a).zip(find(b)).is_some_and(|(a, b)| p[a] == p[b])
}
pub fn ratsnest(board: &Board) -> Vec<(Pad, Pad)> {
    let pads = board.pads();
    let groups = partition(board);
    let mut by_net: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (i, p) in pads.iter().enumerate() {
        if p.net > 0 && p.kind != "np_thru_hole" {
            by_net.entry(p.net).or_default().push(i);
        }
    }
    let mut pairs = vec![];
    for indices in by_net.values() {
        let mut parent: HashMap<usize, usize> = indices.iter().map(|i| (*i, groups[*i])).collect();
        let mut edges = vec![];
        for (a, i) in indices.iter().enumerate() {
            for j in indices.iter().skip(a + 1) {
                edges.push((
                    (pads[*i].at.0 - pads[*j].at.0).hypot(pads[*i].at.1 - pads[*j].at.1),
                    *i,
                    *j,
                ));
            }
        }
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, i, j) in edges {
            let a = parent[&i];
            let b = parent[&j];
            if a != b {
                pairs.push((pads[i].clone(), pads[j].clone()));
                for p in parent.values_mut() {
                    if *p == b {
                        *p = a;
                    }
                }
            }
        }
    }
    pairs
}
pub fn verify(board: &Board, p: &Value) -> Result<Value, anyhow::Error> {
    let c = copper(board);
    let mut findings = vec![];
    let rules = &board.rules;
    let issue = |kind: &str, severity: &str, at: Point, objects: Value, message: String| json!({"kind":kind,"severity":severity,"at_mm":at,"objects":objects,"message":message});
    let edges = board.outline_segments();
    if edges.is_empty() {
        findings.push(issue(
            "outline",
            "error",
            (0., 0.),
            json!([]),
            "Board has no Edge.Cuts outline".into(),
        ));
    } else {
        let mut bad = vec![];
        for (a, b) in &edges {
            for p in [a, b] {
                let count = edges
                    .iter()
                    .filter(|(c, d)| {
                        (p.0 - c.0).hypot(p.1 - c.1) < 0.02 || (p.0 - d.0).hypot(p.1 - d.1) < 0.02
                    })
                    .count();
                if count != 2 {
                    bad.push(*p);
                }
            }
        }
        if !bad.is_empty() {
            findings.push(issue(
                "outline",
                "error",
                bad[0],
                json!([]),
                "Outline contains open ends or branches".into(),
            ));
        }
    }
    for (i, item) in c.iter().enumerate() {
        let center = item.geometry.center();
        let footprint = if item.reference.is_empty() {
            item.uuid.clone()
        } else {
            format!("{}.{}", item.reference, item.pad)
        };
        if item.kind != "hole"
            && (!board.inside(center)
                || edges.iter().any(|(a, b)| {
                    item.geometry.distance(&Geometry::Stroke(*a, *b, 0.))
                        < rules.edge_clearance - EPS
                }))
        {
            findings.push(issue(
                "board_edge",
                "error",
                center,
                json!([item.uuid]),
                format!("{footprint} violates board edge clearance"),
            ));
        }
        if item.net == 0 && item.kind == "pad" {
            findings.push(issue(
                "unassigned_pad",
                "warning",
                center,
                json!([item.uuid]),
                format!("{footprint} has no assigned net"),
            ));
        }
        for other in c.iter().take(i) {
            if item.uuid == other.uuid
                || !item.shares_layer(other)
                || (item.net == other.net && item.net > 0)
                || item.net == 0
                || other.net == 0
            {
                continue;
            }
            let gap = item.geometry.distance(&other.geometry);
            if gap < rules.clearance - EPS {
                let short = gap <= EPS;
                let approximate_zone = !short && (item.kind == "zone" || other.kind == "zone");
                findings.push(issue(
                    if short {
                        "short"
                    } else if approximate_zone {
                        "approximate_zone_clearance"
                    } else {
                        "clearance"
                    },
                    if approximate_zone { "warning" } else { "error" },
                    center,
                    json!([item.uuid, other.uuid]),
                    format!(
                        "{} {} and {} {} on nets {} / {}: copper gap {:.4} mm (required {:.4})",
                        item.reference,
                        item.pad,
                        other.reference,
                        other.pad,
                        item.net,
                        other.net,
                        gap.max(0.),
                        rules.clearance
                    ),
                ));
            }
        }
    }
    for n in board
        .root
        .children("segment")
        .chain(board.root.children("arc"))
    {
        if n.child("width")
            .is_some_and(|n| n.number(1) < rules.track_width - EPS)
        {
            findings.push(issue(
                "track_width",
                "error",
                pcb::point(n, "start"),
                json!([pcb::id(n)]),
                format!("Track is narrower than {:.3} mm", rules.track_width),
            ));
        }
    }
    for n in board.root.children("via") {
        let size = n.child("size").map(|n| n.number(1)).unwrap_or(0.);
        let drill = n.child("drill").map(|n| n.number(1)).unwrap_or(0.);
        if drill <= 0.
            || drill >= size
            || drill < rules.via_drill - EPS
            || size < rules.via_diameter - EPS
        {
            findings.push(issue(
                "via",
                "error",
                pcb::point(n, "at"),
                json!([pcb::id(n)]),
                "Via diameter/drill/annular ring violates the configured rules".into(),
            ));
        }
    }
    for n in board.root.children("zone") {
        if n.child("keepout").is_none() && n.children("filled_polygon").next().is_none() {
            findings.push(issue(
                "unfilled_zone",
                "warning",
                pcb::pts(n.child("polygon").unwrap_or(n))
                    .first()
                    .copied()
                    .unwrap_or((0., 0.)),
                json!([pcb::id(n)]),
                "Copper zone is unfilled; refill before manufacturing export".into(),
            ));
        }
    }
    let mut intent = vec![];
    for (field, wanted) in [
        ("expected_connections", true),
        ("forbidden_connections", false),
    ] {
        for pair in p[field].as_array().into_iter().flatten() {
            match board.pad(&pair["from"]).and_then(|a|board.pad(&pair["to"]).map(|b|connected(board,&a,&b))){Ok(actual)if actual!=wanted=>intent.push(json!({"from":pair["from"],"to":pair["to"],"expected_connected":wanted,"connected":actual})),Err(e)=>intent.push(json!({"from":pair["from"],"to":pair["to"],"error":e.to_string()})),_=>{}}
        }
    }
    let unrouted:Vec<_>=ratsnest(board).into_iter().map(|(a,b)|json!({"net":a.net_name,"from":{"reference":a.reference,"number":a.number,"at_mm":a.at},"to":{"reference":b.reference,"number":b.number,"at_mm":b.at}})).collect();
    let passed = unrouted.is_empty()
        && intent.is_empty()
        && !findings.iter().any(|f| f["severity"] == "error")
        && !findings.iter().any(|f| f["kind"] == "unfilled_zone");
    let total = findings.len();
    let limit = p["limit"].as_u64().unwrap_or(500).min(10000) as usize;
    findings.truncate(limit);
    Ok(
        json!({"revision":board.revision(),"passed":passed,"rules":rules,"findings":findings,"finding_count":total,"truncated":total>limit,"unrouted":unrouted,"connections":{"passed":intent.is_empty(),"findings":intent},"native_roundtrip":crate::sexpr::parse(&board.root.document_text()).is_ok_and(|n|n==board.root),"external_kicad_checked":false,"scope":"board","limitations":["Native KiCad DRC is available through pcb_verify(external:true) for exact custom-pad, zone, courtyard and advanced-rule checking"]}),
    )
}
pub fn safe_track(board: &Board, a: Point, b: Point, width: f64, layer: &str, net: i64) -> bool {
    let g = Geometry::Stroke(a, b, width / 2.);
    if !board.inside(a) || !board.inside(b) || !board.inside(((a.0 + b.0) / 2., (a.1 + b.1) / 2.)) {
        return false;
    }
    if board
        .outline_segments()
        .iter()
        .any(|(c, d)| g.distance(&Geometry::Stroke(*c, *d, 0.)) < board.rules.edge_clearance - EPS)
    {
        return false;
    }
    for item in copper(board) {
        if item.net != net
            && item.layers.iter().any(|l| l == layer || l == "*.Cu")
            && g.distance(&item.geometry) < board.rules.clearance - EPS
        {
            return false;
        }
    }
    for n in board
        .root
        .children("zone")
        .filter(|n| n.child("keepout").is_some() && pcb::layer(n) == layer)
    {
        if let Some(poly) = n.child("polygon") {
            let p = pcb::pts(poly);
            if g.distance(&Geometry::Polygon(p)) < board.rules.clearance - EPS {
                return false;
            }
        }
    }
    true
}
