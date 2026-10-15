use anyhow::{Result, bail};
use jenga_core::{
    checks::on_segment,
    document::Document,
    scene::{Point, Scene, points},
};
/// Conservative orthogonal routes: avoid symbols and accidental T connections.
pub fn route(
    doc: &Document,
    scene: &Scene,
    from: Point,
    to: Point,
    horizontal: bool,
) -> Result<Vec<Point>> {
    route_options(doc, scene, from, to, horizontal, &serde_json::json!({}))
}
pub fn route_options(
    doc: &Document,
    scene: &Scene,
    from: Point,
    to: Point,
    horizontal: bool,
    options: &serde_json::Value,
) -> Result<Vec<Point>> {
    let outward = |at| {
        scene
            .pins
            .iter()
            .find(|p| p.at == at)
            .map(|p| p.outward)
            .unwrap_or((0., 0.))
    };
    let f = outward(from);
    let t = outward(to);
    let a = (from.0 + f.0 * 2.54, from.1 + f.1 * 2.54);
    let b = (to.0 + t.0 * 2.54, to.1 + t.1 * 2.54);
    let ends: Vec<_> = scene
        .pins
        .iter()
        .filter(|p| p.at == from || p.at == to)
        .map(|p| p.object)
        .collect();
    let mut obstacles: Vec<_> = doc
        .root
        .items()
        .iter()
        .enumerate()
        .filter(|(i, n)| n.name() == "symbol" && !ends.contains(i))
        .filter_map(|(i, _)| scene.object_bounds(i))
        .map(|(x, y)| ((x.0 - 1., x.1 - 1.), (y.0 + 1., y.1 + 1.)))
        .collect();
    // Never create half-grid midpoints. Preserve exact imported endpoint coordinates.
    let snap = |v: f64| (v / 1.27).round() * 1.27;
    let mut xs = vec![snap((a.0 + b.0) / 2.), a.0, b.0];
    let mut ys = vec![snap((a.1 + b.1) / 2.), a.1, b.1];
    for (lo, hi) in &obstacles {
        xs.extend([
            ((lo.0 - 2.54) / 1.27).floor() * 1.27,
            ((hi.0 + 2.54) / 1.27).ceil() * 1.27,
        ]);
        ys.extend([
            ((lo.1 - 2.54) / 1.27).floor() * 1.27,
            ((hi.1 + 2.54) / 1.27).ceil() * 1.27,
        ]);
    }
    let nets = jenga_core::connectivity::inspect(doc);
    let mut allowed_ids = std::collections::HashSet::new();
    for net in nets["nets"].as_array().into_iter().flatten() {
        if net["pins"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| p["at_mm"] == serde_json::json!(from) || p["at_mm"] == serde_json::json!(to))
        {
            for id in net["wire_uuids"].as_array().into_iter().flatten() {
                if let Some(id) = id.as_str() {
                    allowed_ids.insert(id.to_owned());
                }
            }
        }
    }
    let allowed_wires: Vec<_> = doc
        .root
        .children("wire")
        .filter(|n| {
            n.child("uuid")
                .is_some_and(|v| allowed_ids.contains(v.value(1)))
        })
        .filter_map(|n| {
            let p = points(n, "pts");
            (p.len() == 2).then(|| (p[0], p[1]))
        })
        .collect();
    let wires: Vec<_> = doc
        .root
        .children("wire")
        .filter_map(|n| {
            let p = points(n, "pts");
            (p.len() == 2).then(|| (p[0], p[1]))
        })
        .collect();
    for (c, d) in &wires {
        for p in [c, d] {
            xs.extend([snap(p.0 - 2.54), snap(p.0 + 2.54)]);
            ys.extend([snap(p.1 - 2.54), snap(p.1 + 2.54)]);
        }
    }
    let protected_points = crate::net_guard::avoid_points(doc);
    let allowed = |p: Point| {
        (p.0 - from.0).hypot(p.1 - from.1) < 1e-5
            || (p.0 - to.0).hypot(p.1 - to.1) < 1e-5
            || allowed_wires.iter().any(|(a, b)| on_segment(p, *a, *b))
    };
    let endpoint = |p: Point| {
        (p.0 - from.0).hypot(p.1 - from.1) < 1e-5 || (p.0 - to.0).hypot(p.1 - to.1) < 1e-5
    };
    for (i, n) in doc.root.items().iter().enumerate() {
        if matches!(
            n.name(),
            "label" | "global_label" | "hierarchical_label" | "text"
        ) {
            let (x, y, _) = jenga_core::document::position(n);
            if !allowed((x, y)) {
                if let Some(bounds) = scene.object_bounds(i) {
                    obstacles.push(bounds);
                }
            }
        }
    }
    for corridor in options["reserved_corridors"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let lo = crate::engine::point(corridor, "from")?;
        let hi = crate::engine::point(corridor, "to")?;
        anyhow::ensure!(
            lo.0 < hi.0 && lo.1 < hi.1,
            "Reserved corridor requires increasing from/to bounds"
        );
        obstacles.push((lo, hi));
    }
    for (lo, hi) in &obstacles {
        xs.extend([snap(lo.0 - 2.54), snap(hi.0 + 2.54)]);
        ys.extend([snap(lo.1 - 2.54), snap(hi.1 + 2.54)]);
    }
    let crossing_cost = |x: Point, y: Point| {
        wires
            .iter()
            .filter(|(c, d)| {
                !allowed_wires.contains(&(*c, *d))
                    && if (x.0 - y.0).abs() < 1e-6 && (c.1 - d.1).abs() < 1e-6 {
                        x.0 > c.0.min(d.0)
                            && x.0 < c.0.max(d.0)
                            && c.1 > x.1.min(y.1)
                            && c.1 < x.1.max(y.1)
                    } else if (x.1 - y.1).abs() < 1e-6 && (c.0 - d.0).abs() < 1e-6 {
                        c.0 > x.0.min(y.0)
                            && c.0 < x.0.max(y.0)
                            && x.1 > c.1.min(d.1)
                            && x.1 < c.1.max(d.1)
                    } else {
                        false
                    }
            })
            .count() as f64
            * 12.7
    };
    let segment_safe = |x: Point, y: Point, graph: bool| {
        if (x.0 - y.0).abs() > 1e-6 && (x.1 - y.1).abs() > 1e-6 {
            return false;
        }
        if obstacles.iter().any(|(lo, hi)| {
            if (x.0 - y.0).abs() < 1e-6 {
                x.0 > lo.0 && x.0 < hi.0 && x.1.max(y.1) > lo.1 && x.1.min(y.1) < hi.1
            } else {
                x.1 > lo.1 && x.1 < hi.1 && x.0.max(y.0) > lo.0 && x.0.min(y.0) < hi.0
            }
        }) {
            return false;
        }
        if scene
            .pins
            .iter()
            .any(|p| p.at != from && p.at != to && on_segment(p.at, x, y))
        {
            return false;
        }
        if protected_points
            .iter()
            .any(|p| on_segment(*p, x, y) && !allowed(*p))
        {
            return false;
        }
        !wires.iter().any(|(c, d)| {
            !allowed_wires.contains(&(*c, *d))
                && ((!graph && on_segment(x, *c, *d) && !endpoint(x))
                    || (!graph && on_segment(y, *c, *d) && !endpoint(y))
                    || (on_segment(*c, x, y) && !endpoint(*c))
                    || (on_segment(*d, x, y) && !endpoint(*d)))
        })
    };
    let safe = |path: &[Point]| {
        if path.len() < 2 {
            return false;
        }
        let start = (path[1].0 - from.0, path[1].1 - from.1);
        let end = (path[path.len() - 2].0 - to.0, path[path.len() - 2].1 - to.1);
        if (f != (0., 0.) && start.0 * f.0 + start.1 * f.1 <= 0.)
            || (t != (0., 0.) && end.0 * t.0 + end.1 * t.1 <= 0.)
        {
            return false;
        }
        path.windows(2).all(|s| segment_safe(s[0], s[1], false))
    };
    let mut candidates = vec![];
    for x in xs.iter().copied() {
        candidates.push(vec![from, a, (x, a.1), (x, b.1), b, to]);
    }
    for y in ys.iter().copied() {
        candidates.push(vec![from, a, (a.0, y), (b.0, y), b, to]);
    }
    candidates.sort_by(|p, q| {
        let length = |v: &Vec<Point>| {
            v.windows(2)
                .map(|w| {
                    (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs() + crossing_cost(w[0], w[1])
                })
                .sum::<f64>()
        };
        length(p).total_cmp(&length(q)).then_with(|| {
            let preferred = |v: &Vec<Point>| {
                if horizontal {
                    v[2].1 == a.1
                } else {
                    v[2].0 == a.0
                }
            };
            preferred(q).cmp(&preferred(p))
        })
    });
    for mut path in candidates {
        path.dedup();
        // Remove only collinear vertices lying between their neighbours: retain U turns.
        let mut i = 1;
        while i + 1 < path.len() {
            if on_segment(path[i], path[i - 1], path[i + 1]) {
                path.remove(i);
            } else {
                i += 1;
            }
        }
        if safe(&path) {
            return Ok(path);
        }
    }
    // A bounded visibility-grid search handles routes requiring more bends.
    // Exact imported endpoint axes coexist with snapped detour axes.
    xs.extend([a.0, b.0, from.0, to.0]);
    ys.extend([a.1, b.1, from.1, to.1]);
    for p in scene
        .pins
        .iter()
        .map(|p| p.at)
        .chain(protected_points.iter().copied())
    {
        xs.extend([snap(p.0 - 2.54), snap(p.0 + 2.54)]);
        ys.extend([snap(p.1 - 2.54), snap(p.1 + 2.54)]);
    }
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    ys.sort_by(f64::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    if xs.len() * ys.len() <= 160000 {
        let ix = |x: f64| xs.iter().position(|v| (*v - x).abs() < 1e-6).unwrap();
        let iy = |y: f64| ys.iter().position(|v| (*v - y).abs() < 1e-6).unwrap();
        let start = (ix(from.0), iy(from.1), 2usize);
        let goal = (ix(to.0), iy(to.1));
        let mut distances = std::collections::HashMap::from([(start, 0_f64)]);
        let mut previous = std::collections::HashMap::new();
        let mut queue = std::collections::BinaryHeap::new();
        queue.push((std::cmp::Reverse(0_i64), start));
        let mut finish = None;
        let mut visits = 0;
        while let Some((_, u)) = queue.pop() {
            if visits % 32 == 0 {
                crate::operations::checkpoint()?;
            }
            visits += 1;
            if visits > 320000 {
                break;
            }
            if (u.0, u.1) == goal {
                finish = Some(u);
                break;
            }
            let p = (xs[u.0], ys[u.1]);
            let base = distances[&u];
            for (dx, dy, axis) in [(-1isize, 0isize, 0usize), (1, 0, 0), (0, -1, 1), (0, 1, 1)] {
                let x = u.0 as isize + dx;
                let y = u.1 as isize + dy;
                if x < 0 || y < 0 || x >= xs.len() as isize || y >= ys.len() as isize {
                    continue;
                }
                let v = (x as usize, y as usize, axis);
                let q = (xs[v.0], ys[v.1]);
                if u == start && f != (0., 0.) && (q.0 - from.0) * f.0 + (q.1 - from.1) * f.1 <= 0.
                {
                    continue;
                }
                if (v.0, v.1) == goal
                    && t != (0., 0.)
                    && (p.0 - to.0) * t.0 + (p.1 - to.1) * t.1 <= 0.
                {
                    continue;
                }

                if u.2 != 2
                    && u.2 != axis
                    && wires
                        .iter()
                        .any(|w| !allowed_wires.contains(w) && on_segment(p, w.0, w.1))
                {
                    continue;
                }
                if !segment_safe(p, q, true) {
                    continue;
                }
                let d = base
                    + (p.0 - q.0).abs()
                    + (p.1 - q.1).abs()
                    + crossing_cost(p, q)
                    + if u.2 != 2 && u.2 != axis { 2.54 } else { 0. };
                if d < *distances.get(&v).unwrap_or(&f64::INFINITY) {
                    distances.insert(v, d);
                    previous.insert(v, u);
                    let h = (q.0 - to.0).abs() + (q.1 - to.1).abs();
                    queue.push((std::cmp::Reverse(((d + h) * 100000.) as i64), v));
                }
            }
        }
        if let Some(mut u) = finish {
            let mut path = vec![];
            while u != start {
                path.push((xs[u.0], ys[u.1]));
                u = previous[&u];
            }
            path.push(from);
            path.reverse();
            if let Some(last) = path.last_mut() {
                *last = to;
            }
            path.dedup();
            let mut i = 1;
            while i + 1 < path.len() {
                if on_segment(path[i], path[i - 1], path[i + 1]) {
                    path.remove(i);
                } else {
                    i += 1;
                }
            }
            if safe(&path) {
                return Ok(path);
            }
        }
    }
    bail!(
        "No safe orthogonal route from {:?} to {:?}; protected pins/labels/endpoints={:?}, obstacles/corridors={:?}. Move components or adjust reserved corridors; no wires were changed.",
        from,
        to,
        protected_points,
        obstacles
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_orthogonal_route() {
        let d = Document::new();
        let s = Scene::from_document(&d);
        let p = route(&d, &s, (1.25, 2.5), (21.75, 15.), true).unwrap();
        assert_eq!(p.first(), Some(&(1.25, 2.5)));
        assert_eq!(p.last(), Some(&(21.75, 15.)));
        assert!(p.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1));
    }
    #[test]
    fn straight_route_has_no_half_grid_midpoint() {
        let d = Document::new();
        let s = Scene::from_document(&d);
        assert_eq!(
            route(&d, &s, (105.41, 71.12), (121.92, 71.12), true).unwrap(),
            vec![(105.41, 71.12), (121.92, 71.12)]
        );
        let p = route(&d, &s, (10.16, 10.16), (26.67, 22.86), true).unwrap();
        assert!(p.iter().all(|p| {
            [p.0, p.1]
                .iter()
                .all(|v| (v / 1.27 - (v / 1.27).round()).abs() < 1e-6)
        }));
    }
    #[test]
    fn avoids_accidental_existing_wire_endpoint_join() {
        let mut d = Document::new();
        d.add_wire((20.32, 10.16), (20.32, 20.32));
        let s = Scene::from_document(&d);
        let path = route(&d, &s, (10.16, 10.16), (30.48, 10.16), true).unwrap();
        assert!(
            path.windows(2)
                .all(|w| !on_segment((20.32, 10.16), w[0], w[1]))
        );
        assert!(path.len() > 2);
    }
}
