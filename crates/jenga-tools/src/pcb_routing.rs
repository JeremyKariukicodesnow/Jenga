//! Bounded obstacle-aware copper routing with through vias between layers.
use anyhow::{Result, bail, ensure};
use jenga_core::{
    pcb::{Board, Pad},
    pcb_checks::{self, Geometry},
    scene::Point,
};
use serde_json::{Value, json};
use std::collections::{BinaryHeap, HashMap};
pub fn connect(board: &mut Board, from: &Value, to: &Value, options: &Value) -> Result<Value> {
    let a = board.pad(from)?;
    let b = board.pad(to)?;
    ensure!(
        a.net > 0 && a.net == b.net,
        "Cannot route {}.{} ({}) to {}.{} ({}): assign matching nets first",
        a.reference,
        a.number,
        a.net_name,
        b.reference,
        b.number,
        b.net_name
    );
    if pcb_checks::connected(board, &a, &b) {
        return Ok(json!({"already_connected":true,"from":from,"to":to}));
    }
    let (path, layers) = route(board, &a, &b, options)?;
    let width = options["width_mm"]
        .as_f64()
        .unwrap_or(board.rules.track_width);
    let mut wires = vec![];
    let mut vias = vec![];
    for i in 1..path.len() {
        if layers[i] != layers[i - 1] {
            vias.push(board.via(path[i], a.net)?);
        } else if path[i] != path[i - 1] {
            wires.push(board.segment(path[i - 1], path[i], width, &layers[i], a.net)?);
        }
    }
    ensure!(
        pcb_checks::connected(board, &a, &b),
        "Route did not connect exact endpoint pads; transaction rejected"
    );
    Ok(
        json!({"from":from,"to":to,"net":a.net_name,"route_mm":path,"layers":layers,"track_uuids":wires,"via_uuids":vias}),
    )
}
pub fn route(
    board: &Board,
    a: &Pad,
    b: &Pad,
    options: &Value,
) -> Result<(Vec<Point>, Vec<String>)> {
    route_points(board, a.at, b.at, a.net, &a.layers, &b.layers, options)
}
pub fn route_points(
    board: &Board,
    from: Point,
    to: Point,
    net: i64,
    from_layers: &[String],
    to_layers: &[String],
    options: &Value,
) -> Result<(Vec<Point>, Vec<String>)> {
    crate::operations::checkpoint()?;
    let started = std::time::Instant::now();
    let budget = std::time::Duration::from_millis(
        options["timeout_ms"]
            .as_u64()
            .unwrap_or(15_000)
            .clamp(100, 120_000),
    );
    let width = options["width_mm"]
        .as_f64()
        .unwrap_or(board.rules.track_width);
    ensure!(
        width >= board.rules.track_width && width.is_finite(),
        "Requested width violates minimum track width"
    );
    let mut layers = board.copper_layers();
    if let Some(layer) = options["layer"].as_str() {
        ensure!(
            layers.iter().any(|l| l == layer),
            "Layer {layer} absent from board"
        );
        layers.retain(|l| l == layer);
    }
    let allowed = |pin: &[String], l: &str| pin.iter().any(|p| p == l || p == "*.Cu");
    ensure!(
        layers.iter().any(|l| allowed(from_layers, l))
            && layers.iter().any(|l| allowed(to_layers, l)),
        "Selected layers cannot reach one endpoint pad"
    );
    let copper = pcb_checks::copper(board);
    let edges = board.outline_segments();
    let keepouts: Vec<_> = board
        .root
        .children("zone")
        .filter(|n| n.child("keepout").is_some())
        .filter_map(|n| {
            n.child("polygon").map(|p| {
                (
                    jenga_core::pcb::layer(n).to_owned(),
                    Geometry::Polygon(jenga_core::pcb::pts(p)),
                )
            })
        })
        .collect();
    let corridors: Vec<_> = options["reserved_corridors"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            let a = jenga_core::pcb::json_point(v, "from")?;
            let b = jenga_core::pcb::json_point(v, "to")?;
            Ok(Geometry::Polygon(vec![a, (b.0, a.1), b, (a.0, b.1)]))
        })
        .collect::<Result<_>>()?;
    let inside = |p: Point| {
        let mut crossings = 0;
        for (a, b) in &edges {
            if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
                crossings += 1;
            }
        }
        crossings % 2 == 1
    };
    let safe = |a: Point, b: Point, l: usize, radius: f64| {
        if !inside(a) || !inside(b) || !inside(((a.0 + b.0) / 2., (a.1 + b.1) / 2.)) {
            return false;
        }
        let g = Geometry::Stroke(a, b, radius);
        if edges.iter().any(|(c, d)| {
            g.distance(&Geometry::Stroke(*c, *d, 0.)) < board.rules.edge_clearance - 1e-6
        }) {
            return false;
        }
        if copper.iter().any(|c| {
            c.net != net
                && c.layers.iter().any(|x| x == &layers[l] || x == "*.Cu")
                && g.near(&c.geometry, board.rules.clearance)
                && g.distance(&c.geometry) < board.rules.clearance - 1e-6
        }) {
            return false;
        }
        if keepouts.iter().any(|(layer, k)| {
            (layer == &layers[l] || layer == "*.Cu") && g.distance(k) < board.rules.clearance
        }) {
            return false;
        }
        !corridors
            .iter()
            .any(|c| g.distance(c) < board.rules.clearance)
    };
    for (l, name) in layers
        .iter()
        .enumerate()
        .filter(|(_, l)| allowed(from_layers, l) && allowed(to_layers, l))
    {
        let candidates = [
            vec![from, to],
            vec![from, (to.0, from.1), to],
            vec![from, (from.0, to.1), to],
            vec![
                from,
                ((from.0 + to.0) / 2., from.1),
                ((from.0 + to.0) / 2., to.1),
                to,
            ],
        ];
        for mut p in candidates {
            p.dedup();
            if p.windows(2).all(|w| safe(w[0], w[1], l, width / 2.)) {
                return Ok((p.clone(), vec![name.clone(); p.len()]));
            }
        }
    }
    let (lo, hi) = board.bounds();
    let grid = options["grid_mm"]
        .as_f64()
        .unwrap_or(board.rules.grid)
        .clamp(0.05, 5.);
    let nx = ((hi.0 - lo.0) / grid).ceil() as i32;
    let ny = ((hi.1 - lo.1) / grid).ceil() as i32;
    ensure!(
        (nx as i64) * (ny as i64) * (layers.len() as i64) <= 2_000_000,
        "Routing grid is too large; use a coarser grid or smaller board"
    );
    let point = |x: i32, y: i32| (lo.0 + x as f64 * grid, lo.1 + y as f64 * grid);
    let snap = |p: Point| {
        (
            ((p.0 - lo.0) / grid).round() as i32,
            ((p.1 - lo.1) / grid).round() as i32,
        )
    };
    let (a, b) = (snap(from), snap(to));
    type State = (i32, i32, usize, u8);
    let mut dist: HashMap<State, f64> = HashMap::new();
    let mut prev: HashMap<State, State> = HashMap::new();
    let mut queue = BinaryHeap::new();
    for (l, layer) in layers.iter().enumerate() {
        if allowed(from_layers, layer) && safe(from, point(a.0, a.1), l, width / 2.) {
            let start = (a.0, a.1, l, 8);
            dist.insert(start, 0.);
            queue.push((std::cmp::Reverse(0_i64), start));
        }
    }
    let directions = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    let mut finish = None;
    let mut visited = 0;
    while let Some((_, u)) = queue.pop() {
        if visited % 16 == 0 {
            crate::operations::checkpoint()?;
            ensure!(
                started.elapsed() < budget,
                "Routing time budget exhausted for net {} from {:?} to {:?}; no route applied. Inspect blocking copper or provide a reserved corridor",
                board.nets().get(&net).cloned().unwrap_or_default(),
                from,
                to
            );
        }
        if visited % 1024 == 0 {
            crate::operations::progress("routing", visited);
        }
        let base = dist[&u];
        visited += 1;
        if visited > 500_000 {
            break;
        }
        let p = point(u.0, u.1);
        if (u.0, u.1) == b && allowed(to_layers, &layers[u.2]) && safe(p, to, u.2, width / 2.) {
            finish = Some(u);
            break;
        }
        let mut neighbors = vec![];
        for (dir, (dx, dy)) in directions.iter().enumerate() {
            let v = (u.0 + dx, u.1 + dy, u.2, dir as u8);
            if v.0 >= 0 && v.1 >= 0 && v.0 <= nx && v.1 <= ny {
                neighbors.push((
                    v,
                    grid * if *dx != 0 && *dy != 0 {
                        2_f64.sqrt()
                    } else {
                        1.
                    } + (if u.3 != 8 && u.3 != dir as u8 {
                        grid * 0.3
                    } else {
                        0.
                    }),
                    false,
                ));
            }
        }
        if options["allow_vias"] != false {
            for l in 0..layers.len() {
                if l != u.2 {
                    neighbors.push(((u.0, u.1, l, 8), 5., true));
                }
            }
        }
        for (v, cost, via) in neighbors {
            let q = point(v.0, v.1);
            let valid = if via {
                (0..layers.len()).all(|l| safe(p, p, l, board.rules.via_diameter / 2.))
            } else {
                safe(p, q, u.2, width / 2.)
            };
            if !valid {
                continue;
            }
            let next = base + cost;
            if next + 1e-9 < *dist.get(&v).unwrap_or(&f64::INFINITY) {
                dist.insert(v, next);
                prev.insert(v, u);
                let h = (q.0 - to.0).hypot(q.1 - to.1);
                queue.push((std::cmp::Reverse(((next + h) * 100000.) as i64), v));
            }
        }
    }
    if let Some(mut u) = finish {
        let mut states = vec![u];
        while let Some(p) = prev.get(&u) {
            u = *p;
            states.push(u);
        }
        states.reverse();
        let mut path = vec![from];
        let mut route_layers = vec![layers[states[0].2].clone()];
        for state in states {
            let p = point(state.0, state.1);
            let l = layers[state.2].clone();
            if path.last() != Some(&p) || route_layers.last() != Some(&l) {
                path.push(p);
                route_layers.push(l);
            }
        }
        if path.last() != Some(&to) {
            path.push(to);
            route_layers.push(route_layers.last().unwrap().clone());
        }
        let mut i = 1;
        while i + 1 < path.len() {
            if route_layers[i - 1] == route_layers[i]
                && route_layers[i] == route_layers[i + 1]
                && pcb_checks::point_segment(path[i], path[i - 1], path[i + 1]) < 1e-7
            {
                path.remove(i);
                route_layers.remove(i);
            } else {
                i += 1;
            }
        }
        return Ok((path, route_layers));
    }
    bail!(
        "No clearance-safe PCB route from {:?} to {:?} on net {} after {} search states. Obstacles: {}. Adjust placement, constraints or reserved corridors",
        from,
        to,
        board.nets().get(&net).unwrap_or(&String::new()),
        visited,
        copper
            .iter()
            .filter(|c| c.net != net)
            .map(|c| format!("{}:{}({})", c.reference, c.pad, c.uuid))
            .take(24)
            .collect::<Vec<_>>()
            .join(", ")
    )
}
