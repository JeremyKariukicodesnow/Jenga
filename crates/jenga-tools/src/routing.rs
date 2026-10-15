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
    let obstacles: Vec<_> = doc
        .root
        .items()
        .iter()
        .enumerate()
        .filter(|(i, n)| n.name() == "symbol" && !ends.contains(i))
        .filter_map(|(i, _)| scene.object_bounds(i))
        .map(|(x, y)| ((x.0 - 1., x.1 - 1.), (y.0 + 1., y.1 + 1.)))
        .collect();
    let mut xs = vec![(a.0 + b.0) / 2., a.0, b.0];
    let mut ys = vec![(a.1 + b.1) / 2., a.1, b.1];
    for (lo, hi) in &obstacles {
        xs.extend([lo.0 - 2.54, hi.0 + 2.54]);
        ys.extend([lo.1 - 2.54, hi.1 + 2.54]);
    }
    let wires: Vec<_> = doc
        .root
        .children("wire")
        .filter_map(|n| {
            let p = points(n, "pts");
            (p.len() == 2).then(|| (p[0], p[1]))
        })
        .collect();
    let safe = |path: &[Point]| {
        path.windows(2).all(|s| {
            let (x, y) = (s[0], s[1]);
            if x.0 != y.0 && x.1 != y.1 {
                return false;
            }
            if obstacles.iter().any(|(lo, hi)| {
                if x.0 == y.0 {
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
            !wires.iter().any(|(c, d)| {
                !on_segment(from, *c, *d)
                    && !on_segment(to, *c, *d)
                    && (on_segment(x, *c, *d) || on_segment(y, *c, *d))
            })
        })
    };
    let mut candidates = vec![];
    for x in xs {
        candidates.push(vec![from, a, (x, a.1), (x, b.1), b, to]);
    }
    for y in ys {
        candidates.push(vec![from, a, (a.0, y), (b.0, y), b, to]);
    }
    candidates.sort_by(|p, q| {
        let length = |v: &Vec<Point>| {
            v.windows(2)
                .map(|w| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs())
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
        if safe(&path) {
            return Ok(path);
        }
    }
    bail!(
        "No safe simple route. Move components apart or add explicit wire segments and intended junctions; no wires were changed."
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
}
