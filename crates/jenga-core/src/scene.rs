//! A shared millimetre-space scene for the canvas and vector exports.
use crate::{
    document::{Document, position, property},
    sexpr::Node,
};

pub type Point = (f64, f64);
pub const BODY: [u8; 3] = [132, 0, 0];
pub const WIRE: [u8; 3] = [0, 132, 0];
pub const TEXT: [u8; 3] = [0, 83, 130];
#[derive(Clone, Debug)]
pub enum Shape {
    Path {
        points: Vec<Point>,
        closed: bool,
        fill: Option<[u8; 3]>,
        color: [u8; 3],
        width: f64,
    },
    Circle {
        center: Point,
        radius: f64,
        fill: Option<[u8; 3]>,
        color: [u8; 3],
        width: f64,
    },
    Text {
        at: Point,
        text: String,
        size: f64,
        angle: f64,
        color: [u8; 3],
        left: bool,
    },
}
#[derive(Clone, Debug)]
pub struct Draw {
    pub shape: Shape,
    pub object: Option<usize>,
}
#[derive(Clone, Debug)]
pub struct Pin {
    pub at: Point,
    pub reference: String,
    pub number: String,
    pub kind: String,
    pub name: String,
    pub hidden: bool,
    pub outward: Point,
    pub object: usize,
}
#[derive(Default, Clone)]
pub struct Scene {
    pub draws: Vec<Draw>,
    pub pins: Vec<Pin>,
    pub missing: Vec<String>,
}
impl Shape {
    pub fn bounds(&self) -> (Point, Point) {
        match self {
            Shape::Path { points, .. } => bounds(points),
            Shape::Circle {
                center: (x, y),
                radius: r,
                ..
            } => ((*x - r, *y - r), (*x + r, *y + r)),
            Shape::Text {
                at: (x, y),
                text,
                size,
                left,
                ..
            } => {
                let w = text.chars().count() as f64 * size * 0.65;
                let x0 = if *left { *x } else { *x - w / 2. };
                ((x0, *y - size), (x0 + w, *y + size))
            }
        }
    }
}
pub fn bounds(points: &[Point]) -> (Point, Point) {
    let mut min = (f64::INFINITY, f64::INFINITY);
    let mut max = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &(x, y) in points {
        min.0 = min.0.min(x);
        min.1 = min.1.min(y);
        max.0 = max.0.max(x);
        max.1 = max.1.max(y);
    }
    (min, max)
}
impl Scene {
    /// Shared page decoration keeps printed exports and the editing canvas consistent.
    pub fn page_document(doc: &Document) -> Self {
        let mut scene = Self::from_document(doc);
        crate::page::worksheet(doc, &mut scene);
        scene
    }
    /// Two-dimensional footprint preview, sharing the schematic's vector renderer.
    pub fn footprint_preview(root: &Node) -> Self {
        let mut scene = Self::default();
        for n in root.items() {
            let color = if n
                .child("layer")
                .is_some_and(|l| l.value(1).contains("CrtYd"))
            {
                [110, 110, 120]
            } else {
                BODY
            };
            match n.name() {
                "fp_line" => scene.path(
                    vec![coords(n, "start"), coords(n, "end")],
                    false,
                    None,
                    color,
                    0.12,
                    None,
                ),
                "fp_rect" => {
                    let a = coords(n, "start");
                    let b = coords(n, "end");
                    scene.path(
                        vec![a, (b.0, a.1), b, (a.0, b.1)],
                        true,
                        None,
                        color,
                        0.12,
                        None,
                    );
                }
                "fp_circle" => {
                    let a = coords(n, "center");
                    let b = coords(n, "end");
                    scene.circle(a, (a.0 - b.0).hypot(a.1 - b.1), None, color, 0.12, None);
                }
                "fp_arc" => scene.path(
                    arc_points(coords(n, "start"), coords(n, "mid"), coords(n, "end")),
                    false,
                    None,
                    color,
                    0.12,
                    None,
                ),
                "fp_poly" => scene.path(points(n, "pts"), true, None, color, 0.12, None),
                "pad" => {
                    let (x, y, angle) = position(n);
                    let size = coords(n, "size");
                    let a = angle.to_radians();
                    let transform = |(px, py): Point| {
                        (
                            x + px * a.cos() - py * a.sin(),
                            y + px * a.sin() + py * a.cos(),
                        )
                    };
                    if n.value(3) == "circle" {
                        scene.circle((x, y), size.0 / 2., Some([218, 166, 70]), BODY, 0.1, None);
                    } else {
                        scene.path(
                            vec![
                                transform((-size.0 / 2., -size.1 / 2.)),
                                transform((size.0 / 2., -size.1 / 2.)),
                                transform((size.0 / 2., size.1 / 2.)),
                                transform((-size.0 / 2., size.1 / 2.)),
                            ],
                            true,
                            Some([218, 166, 70]),
                            BODY,
                            0.1,
                            None,
                        );
                    }
                    if let Some(drill) = n.child("drill") {
                        let d = drill.number(if drill.value(1) == "oval" { 2 } else { 1 });
                        scene.circle((x, y), d / 2., Some([250, 249, 244]), BODY, 0.08, None);
                    }
                    scene.text((x, y), n.value(1).into(), 0.65, 0., TEXT, false, None);
                }
                _ => {}
            }
        }
        scene
    }
    pub fn from_document(doc: &Document) -> Self {
        let mut scene = Self::default();
        for (index, n) in doc.root.items().iter().enumerate().skip(1) {
            let object = Some(index);
            match n.name() {
                "wire" | "bus" | "polyline" => {
                    let points = points(n, "pts");
                    let color = if n.name() == "wire" { WIRE } else { TEXT };
                    scene.path(points, false, None, color, 0.15, object);
                }
                "junction" => {
                    let (x, y, _) = position(n);
                    scene.circle(
                        (x, y),
                        n.child("diameter")
                            .map(|d| d.number(1))
                            .filter(|d| *d > 0.)
                            .unwrap_or(0.9)
                            / 2.,
                        Some(WIRE),
                        WIRE,
                        0.1,
                        object,
                    );
                }
                "no_connect" => {
                    let (x, y, _) = position(n);
                    scene.path(
                        vec![(x - 0.6, y - 0.6), (x + 0.6, y + 0.6)],
                        false,
                        None,
                        TEXT,
                        0.15,
                        object,
                    );
                    scene.path(
                        vec![(x - 0.6, y + 0.6), (x + 0.6, y - 0.6)],
                        false,
                        None,
                        TEXT,
                        0.15,
                        object,
                    );
                }
                "label" | "global_label" | "hierarchical_label" | "text" => {
                    scene.node_text(n, n.value(1), TEXT, object);
                }
                "symbol" => {
                    let lib_id = n.child("lib_id").map(|n| n.value(1)).unwrap_or("");
                    if let Some(def) = doc.instance_definition(n) {
                        scene.symbol(def, n, index);
                    } else {
                        scene.missing.push(lib_id.to_owned());
                        let (x, y, _) = position(n);
                        scene.path(
                            vec![
                                (x - 3., y - 3.),
                                (x + 3., y - 3.),
                                (x + 3., y + 3.),
                                (x - 3., y + 3.),
                            ],
                            true,
                            None,
                            BODY,
                            0.3,
                            object,
                        );
                        scene.text(
                            (x, y),
                            format!("Missing {lib_id}"),
                            1.,
                            0.,
                            BODY,
                            false,
                            object,
                        );
                    }
                    for p in n.children("property") {
                        if !hidden(p) {
                            scene.node_text(p, p.value(2), TEXT, object);
                        }
                    }
                }
                "sheet" => {
                    let (x, y, _) = position(n);
                    let size = n
                        .child("size")
                        .map(|s| (s.number(1), s.number(2)))
                        .unwrap_or((20., 15.));
                    scene.path(
                        vec![
                            (x, y),
                            (x + size.0, y),
                            (x + size.0, y + size.1),
                            (x, y + size.1),
                        ],
                        true,
                        None,
                        TEXT,
                        0.2,
                        object,
                    );
                    scene.text(
                        (x + 1., y + 2.),
                        property(n, "Sheetname").to_owned(),
                        1.27,
                        0.,
                        TEXT,
                        true,
                        object,
                    );
                }
                _ => {}
            }
        }
        scene
    }
    pub fn symbol_preview(def: &Node) -> Self {
        Self::symbol_preview_unit(def, 1)
    }
    pub fn symbol_preview_unit(def: &Node, unit: u32) -> Self {
        let mut doc = Document::new();
        let index = doc.add_symbol("preview:Symbol", def, 0., 0., unit);
        doc.set_property(
            index,
            "Reference",
            property(def, "Reference").trim_end_matches('?'),
        );
        Self::from_document(&doc)
    }
    pub fn symbol_body_bounds(def: &Node, unit: u32) -> (Point, Point) {
        let instance = Node::list(
            "symbol",
            vec![
                crate::document::at(0., 0., 0.),
                Node::list("unit", vec![Node::atom(unit)]),
            ],
        );
        let mut scene = Self::default();
        scene.symbol(def, &instance, 0);
        scene.content_bounds()
    }
    fn symbol(&mut self, def: &Node, instance: &Node, index: usize) {
        let (x, y, angle) = position(instance);
        let a = angle.to_radians();
        let unit = instance
            .child("unit")
            .map(|n| n.number(1) as i32)
            .unwrap_or(1);
        let mirror = instance.child("mirror").map(|n| n.value(1)).unwrap_or("");
        let transform = |p: Point| {
            let (mut px, mut py) = p;
            if mirror == "x" {
                py = -py;
            }
            if mirror == "y" {
                px = -px;
            }
            (
                x + px * a.cos() - py * a.sin(),
                y - px * a.sin() - py * a.cos(),
            )
        };
        let obj = Some(index);
        let names_hidden = def.child("pin_names").is_some_and(hidden);
        let numbers_hidden = def.child("pin_numbers").is_some_and(hidden);
        let offset = def
            .child("pin_names")
            .and_then(|n| n.child("offset"))
            .map(|n| n.number(1))
            .unwrap_or(0.508);
        for block in def.children("symbol") {
            let parts: Vec<_> = block.value(1).rsplit('_').take(2).collect();
            let block_unit = parts
                .get(1)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            let style = parts
                .first()
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            if block_unit != 0 && block_unit != unit || style > 1 {
                continue;
            }
            for g in block.items().iter().skip(2) {
                let width = g
                    .child("stroke")
                    .and_then(|n| n.child("width"))
                    .map(|n| n.number(1))
                    .filter(|v| *v > 0.)
                    .unwrap_or(0.15);
                let fill =
                    g.child("fill")
                        .and_then(|n| n.child("type"))
                        .and_then(|t| match t.value(1) {
                            "background" => Some([255, 255, 194]),
                            "outline" => Some(BODY),
                            _ => None,
                        });
                match g.name() {
                    "rectangle" => {
                        let p = coords(g, "start");
                        let q = coords(g, "end");
                        self.path(
                            vec![
                                transform(p),
                                transform((q.0, p.1)),
                                transform(q),
                                transform((p.0, q.1)),
                            ],
                            true,
                            fill,
                            BODY,
                            width,
                            obj,
                        );
                    }
                    "polyline" => {
                        let ps = points(g, "pts").into_iter().map(transform).collect();
                        self.path(ps, fill.is_some(), fill, BODY, width, obj);
                    }
                    "circle" => self.circle(
                        transform(coords(g, "center")),
                        g.child("radius").map(|n| n.number(1)).unwrap_or(1.),
                        fill,
                        BODY,
                        width,
                        obj,
                    ),
                    "arc" => {
                        let ps = arc_points(coords(g, "start"), coords(g, "mid"), coords(g, "end"));
                        self.path(
                            ps.into_iter().map(transform).collect(),
                            false,
                            fill,
                            BODY,
                            width,
                            obj,
                        );
                    }
                    "bezier" => {
                        let ps = points(g, "pts");
                        if ps.len() == 4 {
                            let curve = (0..=32)
                                .map(|i| {
                                    let t = i as f64 / 32.;
                                    let u = 1. - t;
                                    transform((
                                        u * u * u * ps[0].0
                                            + 3. * u * u * t * ps[1].0
                                            + 3. * u * t * t * ps[2].0
                                            + t * t * t * ps[3].0,
                                        u * u * u * ps[0].1
                                            + 3. * u * u * t * ps[1].1
                                            + 3. * u * t * t * ps[2].1
                                            + t * t * t * ps[3].1,
                                    ))
                                })
                                .collect();
                            self.path(curve, false, fill, BODY, width, obj);
                        }
                    }
                    "text" => {
                        let (tx, ty, ta) = position(g);
                        self.text(
                            transform((tx, ty)),
                            g.value(1).into(),
                            font_size(g),
                            ta + angle,
                            BODY,
                            false,
                            obj,
                        );
                    }
                    "pin" => {
                        let (px, py, pa) = position(g);
                        let length = g.child("length").map(|n| n.number(1)).unwrap_or(2.54);
                        let pa = pa.to_radians();
                        let p = (px, py);
                        let q = (px + length * pa.cos(), py + length * pa.sin());
                        let start = transform(p);
                        let end = transform(q);
                        self.pins.push(Pin {
                            at: start,
                            reference: property(instance, "Reference").into(),
                            number: g.child("number").map(|n| n.value(1)).unwrap_or("").into(),
                            kind: g.value(1).into(),
                            name: g.child("name").map(|n| n.value(1)).unwrap_or("").into(),
                            hidden: hidden(g),
                            outward: {
                                let unit_end = transform((px + pa.cos(), py + pa.sin()));
                                (start.0 - unit_end.0, start.1 - unit_end.1)
                            },
                            object: index,
                        });
                        if hidden(g) {
                            continue;
                        }
                        self.path(vec![start, end], false, None, BODY, 0.15, obj);
                        let style = g.value(2);
                        if style.contains("inverted") {
                            self.circle(end, 0.45, None, BODY, 0.15, obj);
                        }
                        if style.contains("clock") {
                            let dx = pa.cos();
                            let dy = pa.sin();
                            self.path(
                                vec![
                                    transform((q.0 - dy * 0.6, q.1 + dx * 0.6)),
                                    transform((q.0 + dx * 0.9, q.1 + dy * 0.9)),
                                    transform((q.0 + dy * 0.6, q.1 - dx * 0.6)),
                                ],
                                false,
                                None,
                                BODY,
                                0.15,
                                obj,
                            );
                        }
                        if !numbers_hidden && let Some(number) = g.child("number") {
                            let vertical = (end.1 - start.1).abs() > (end.0 - start.0).abs();
                            self.text(
                                (
                                    (start.0 + end.0) / 2. - if vertical { 0.7 } else { 0. },
                                    (start.1 + end.1) / 2. - if vertical { 0. } else { 0.7 },
                                ),
                                number.value(1).into(),
                                font_size(number).min(1.27),
                                if vertical { 90. } else { 0. },
                                BODY,
                                false,
                                obj,
                            );
                        }
                        if !names_hidden
                            && let Some(name) = g.child("name")
                            && name.value(1) != "~"
                        {
                            let size = font_size(name).min(1.27);
                            let strokes = crate::font::paths(name.value(1), size, false);
                            let (a, b) =
                                bounds(&strokes.iter().flatten().copied().collect::<Vec<_>>());
                            let width = if a.0.is_finite() { b.0 - a.0 } else { 0. };
                            let length = (end.0 - start.0).hypot(end.1 - start.1).max(1e-6);
                            let dx = (end.0 - start.0) / length;
                            let dy = (end.1 - start.1) / length;
                            self.text(
                                (
                                    end.0 + dx * (offset + width / 2.),
                                    end.1 + dy * (offset + width / 2.),
                                ),
                                name.value(1).into(),
                                size,
                                if dy.abs() > dx.abs() { 90. } else { 0. },
                                TEXT,
                                false,
                                obj,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    fn node_text(&mut self, n: &Node, text: &str, color: [u8; 3], object: Option<usize>) {
        let (x, y, a) = position(n);
        let left = n
            .child("effects")
            .and_then(|e| e.child("justify"))
            .is_some_and(|j| j.items().iter().any(|v| v.text() == "left"));
        let right = n
            .child("effects")
            .and_then(|e| e.child("justify"))
            .is_some_and(|j| j.items().iter().any(|v| v.text() == "right"));
        let size = font_size(n);
        let shift = if right {
            text.chars().count() as f64 * size * 0.54
        } else {
            0.
        };
        let radians = a.to_radians();
        self.text(
            (x - shift * radians.cos(), y - shift * radians.sin()),
            text.replace("~{", "").replace('}', ""),
            size,
            a,
            color,
            left || right,
            object,
        );
    }
    // Scene builders keep geometry, appearance and document identity together.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        at: Point,
        text: String,
        size: f64,
        angle: f64,
        color: [u8; 3],
        left: bool,
        object: Option<usize>,
    ) {
        let a = angle.to_radians();
        for stroke in crate::font::paths(&text, size, left) {
            let points = stroke
                .into_iter()
                .map(|(x, y)| {
                    (
                        at.0 + x * a.cos() + y * a.sin(),
                        at.1 - x * a.sin() + y * a.cos(),
                    )
                })
                .collect();
            self.path(points, false, None, color, size * 0.1, object);
        }
    }
    pub fn path(
        &mut self,
        points: Vec<Point>,
        closed: bool,
        fill: Option<[u8; 3]>,
        color: [u8; 3],
        width: f64,
        object: Option<usize>,
    ) {
        if points.len() > 1 {
            self.draws.push(Draw {
                shape: Shape::Path {
                    points,
                    closed,
                    fill,
                    color,
                    width,
                },
                object,
            });
        }
    }
    pub fn circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<[u8; 3]>,
        color: [u8; 3],
        width: f64,
        object: Option<usize>,
    ) {
        self.draws.push(Draw {
            shape: Shape::Circle {
                center,
                radius,
                fill,
                color,
                width,
            },
            object,
        });
    }
    pub fn object_bounds(&self, index: usize) -> Option<(Point, Point)> {
        let mut ps = vec![];
        for draw in self.draws.iter().filter(|d| d.object == Some(index)) {
            let (a, b) = draw.shape.bounds();
            ps.extend([a, b]);
        }
        if ps.is_empty() {
            None
        } else {
            Some(bounds(&ps))
        }
    }
    pub fn content_bounds(&self) -> (Point, Point) {
        let mut ps = vec![];
        for d in &self.draws {
            let (a, b) = d.shape.bounds();
            ps.extend([a, b]);
        }
        if ps.is_empty() {
            ((0., 0.), (297., 210.))
        } else {
            bounds(&ps)
        }
    }
}
pub fn hidden(n: &Node) -> bool {
    n.items().iter().any(|v| v.text() == "hide")
        || n.child("hide").is_some_and(|h| h.value(1) != "no")
        || n.child("effects").is_some_and(hidden)
}
pub fn font_size(n: &Node) -> f64 {
    n.child("effects")
        .and_then(|e| e.child("font"))
        .and_then(|f| f.child("size"))
        .map(|s| s.number(2))
        .filter(|s| *s > 0.)
        .unwrap_or(1.27)
}
pub fn coords(n: &Node, key: &str) -> Point {
    n.child(key)
        .map(|p| (p.number(1), p.number(2)))
        .unwrap_or_default()
}
pub fn points(n: &Node, key: &str) -> Vec<Point> {
    n.child(key)
        .map(|pts| {
            pts.children("xy")
                .map(|p| (p.number(1), p.number(2)))
                .collect()
        })
        .unwrap_or_default()
}
pub fn arc_points(a: Point, b: Point, c: Point) -> Vec<Point> {
    let d = 2. * (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1));
    if d.abs() < 1e-9 {
        return vec![a, b, c];
    }
    let sq = |p: Point| p.0 * p.0 + p.1 * p.1;
    let center = (
        (sq(a) * (b.1 - c.1) + sq(b) * (c.1 - a.1) + sq(c) * (a.1 - b.1)) / d,
        (sq(a) * (c.0 - b.0) + sq(b) * (a.0 - c.0) + sq(c) * (b.0 - a.0)) / d,
    );
    let angle = |p: Point| (p.1 - center.1).atan2(p.0 - center.0);
    let start = angle(a);
    let tau = std::f64::consts::TAU;
    let mut sweep = (angle(c) - start).rem_euclid(tau);
    if (angle(b) - start).rem_euclid(tau) > sweep {
        sweep -= tau;
    }
    let r = ((a.0 - center.0).powi(2) + (a.1 - center.1).powi(2)).sqrt();
    (0..=48)
        .map(|i| {
            let t = start + sweep * i as f64 / 48.;
            (center.0 + r * t.cos(), center.1 + r * t.sin())
        })
        .collect()
}
