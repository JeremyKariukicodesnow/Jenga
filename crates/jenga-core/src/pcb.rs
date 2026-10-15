//! Native KiCad PCB/footprint data. Unknown fields remain in the semantic AST.
use crate::{
    document::{atomic_write, normalized_path},
    scene::{Draw, Point, Scene, Shape},
    sexpr::{Node, parse},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub fn node(name: &str, values: Vec<Node>) -> Node {
    Node::list(name, values)
}
pub fn xy(name: &str, p: Point) -> Node {
    node(name, vec![Node::atom(p.0), Node::atom(p.1)])
}
pub fn uid() -> Node {
    node("uuid", vec![Node::string(uuid::Uuid::new_v4())])
}
pub fn id(n: &Node) -> &str {
    n.child("uuid")
        .or_else(|| n.child("tstamp"))
        .map(|n| n.value(1))
        .unwrap_or("")
}
pub fn point(n: &Node, key: &str) -> Point {
    n.child(key)
        .map(|p| (p.number(1), p.number(2)))
        .unwrap_or((0., 0.))
}
pub fn angle(n: &Node) -> f64 {
    n.child("at").map(|p| p.number(3)).unwrap_or(0.)
}
pub fn layer(n: &Node) -> &str {
    n.child("layer").map(|n| n.value(1)).unwrap_or("F.Cu")
}
pub fn field<'a>(n: &'a Node, key: &str) -> &'a str {
    n.children("property")
        .find(|p| p.value(1) == key)
        .map(|p| p.value(2))
        .or_else(|| {
            n.children("fp_text")
                .find(|p| p.value(1).eq_ignore_ascii_case(key))
                .map(|p| p.value(2))
        })
        .unwrap_or("")
}
pub fn set_field(n: &mut Node, key: &str, value: &str) {
    if let Some(p) = n
        .items_mut()
        .iter_mut()
        .find(|p| p.name() == "property" && p.value(1) == key)
    {
        p.items_mut()[2] = Node::string(value);
    } else if let Some(p) = n
        .items_mut()
        .iter_mut()
        .find(|p| p.name() == "fp_text" && p.value(1).eq_ignore_ascii_case(key))
    {
        p.items_mut()[2] = Node::string(value);
    } else {
        n.items_mut().push(node(
            "property",
            vec![
                Node::string(key),
                Node::string(value),
                node(
                    "at",
                    vec![
                        Node::atom(0),
                        Node::atom(if key == "Reference" { -3 } else { 3 }),
                        Node::atom(0),
                    ],
                ),
                node("layer", vec![Node::string("F.SilkS")]),
                node(
                    "effects",
                    vec![node(
                        "font",
                        vec![
                            node("size", vec![Node::atom(1), Node::atom(1)]),
                            node("thickness", vec![Node::atom(0.15)]),
                        ],
                    )],
                ),
            ],
        ));
    }
}
pub fn transform(p: Point, at: Point, angle: f64) -> Point {
    let r = angle.to_radians();
    (
        at.0 + p.0 * r.cos() + p.1 * r.sin(),
        at.1 - p.0 * r.sin() + p.1 * r.cos(),
    )
}
pub fn net_code(n: &Node) -> i64 {
    n.child("net")
        .map(|n| match n.items().get(1) {
            Some(Node::String(name)) => named_net_code(name),
            _ => n.number(1) as i64,
        })
        .unwrap_or(0)
}
pub fn named_net_code(name: &str) -> i64 {
    if name.is_empty() {
        0
    } else {
        (u64::from_str_radix(&revision_text(name), 16).unwrap() & 0x000f_ffff_ffff_ffff) as i64 + 1
    }
}
pub fn pts(n: &Node) -> Vec<Point> {
    n.child("pts")
        .map(|p| {
            p.children("xy")
                .map(|n| (n.number(1), n.number(2)))
                .collect()
        })
        .unwrap_or_default()
}
pub fn revision_text(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for b in text.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Rules {
    pub clearance: f64,
    pub track_width: f64,
    pub via_diameter: f64,
    pub via_drill: f64,
    pub edge_clearance: f64,
    pub grid: f64,
}
impl Default for Rules {
    fn default() -> Self {
        Self {
            clearance: 0.2,
            track_width: 0.25,
            via_diameter: 0.6,
            via_drill: 0.3,
            edge_clearance: 0.25,
            grid: 0.5,
        }
    }
}
impl Rules {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            [
                self.clearance,
                self.track_width,
                self.via_diameter,
                self.via_drill,
                self.edge_clearance,
                self.grid
            ]
            .iter()
            .all(|v| v.is_finite() && *v > 0. && *v < 100.),
            "PCB rules require finite positive millimetres below 100"
        );
        ensure!(
            self.via_diameter > self.via_drill,
            "Via diameter must exceed drill diameter"
        );
        Ok(())
    }
}
#[derive(Clone)]
pub struct Board {
    pub root: Node,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub rules: Rules,
    pub schematic: Option<PathBuf>,
    pub extra_nets: BTreeMap<i64, String>,
    pub undo: Vec<BoardState>,
    pub redo: Vec<BoardState>,
    disk_revision: Option<String>,
    project_revision: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Pad {
    pub reference: String,
    pub footprint_uuid: String,
    pub uuid: String,
    pub number: String,
    pub kind: String,
    pub shape: String,
    pub at: Point,
    pub size: Point,
    pub angle: f64,
    pub layers: Vec<String>,
    pub net: i64,
    pub net_name: String,
    pub drill: f64,
    pub object: usize,
}
impl Pad {
    pub fn on_layer(&self, l: &str) -> bool {
        self.kind != "np_thru_hole" && self.layers.iter().any(|s| s == l || s == "*.Cu")
    }
}
#[derive(Clone)]
pub struct BoardState {
    pub root: Node,
    pub rules: Rules,
    pub schematic: Option<PathBuf>,
    pub extra_nets: BTreeMap<i64, String>,
}
impl Board {
    pub fn state(&self) -> BoardState {
        BoardState {
            root: self.root.clone(),
            rules: self.rules.clone(),
            schematic: self.schematic.clone(),
            extra_nets: self.extra_nets.clone(),
        }
    }
    pub fn snapshot(&self) -> Self {
        Self {
            root: self.root.clone(),
            path: self.path.clone(),
            dirty: self.dirty,
            rules: self.rules.clone(),
            schematic: self.schematic.clone(),
            extra_nets: self.extra_nets.clone(),
            undo: vec![],
            redo: vec![],
            disk_revision: self.disk_revision.clone(),
            project_revision: self.project_revision.clone(),
        }
    }
    pub fn restore(&mut self, state: BoardState) {
        self.root = state.root;
        self.rules = state.rules;
        self.schematic = state.schematic;
        self.extra_nets = state.extra_nets;
        self.dirty = true;
    }
    pub fn place_footprint(&mut self, i: usize, at: Point, rotation: f64) {
        let delta = rotation - angle(&self.root.items()[i]);
        let f = &mut self.root.items_mut()[i];
        for n in f
            .items_mut()
            .iter_mut()
            .filter(|n| matches!(n.name(), "pad" | "property" | "fp_text"))
        {
            let p = point(n, "at");
            let r = angle(n) + delta;
            n.set_child(node(
                "at",
                vec![Node::atom(p.0), Node::atom(p.1), Node::atom(r)],
            ));
        }
        f.set_child(node(
            "at",
            vec![Node::atom(at.0), Node::atom(at.1), Node::atom(rotation)],
        ));
        self.dirty = true;
    }

    pub fn new(width: f64, height: f64) -> Result<Self> {
        ensure!(
            width.is_finite()
                && height.is_finite()
                && width >= 5.
                && height >= 5.
                && width <= 2000.
                && height <= 2000.,
            "Board width/height must be 5–2000 mm"
        );
        let mut layers = vec![];
        for (i, name, kind) in [
            (0, "F.Cu", "signal"),
            (31, "B.Cu", "signal"),
            (34, "B.Paste", "user"),
            (35, "F.Paste", "user"),
            (36, "B.SilkS", "user"),
            (37, "F.SilkS", "user"),
            (38, "B.Mask", "user"),
            (39, "F.Mask", "user"),
            (44, "Edge.Cuts", "user"),
            (46, "B.CrtYd", "user"),
            (47, "F.CrtYd", "user"),
            (48, "B.Fab", "user"),
            (49, "F.Fab", "user"),
        ] {
            layers.push(Node::List(vec![
                Node::atom(i),
                Node::string(name),
                Node::atom(kind),
            ]));
        }
        let root = node(
            "kicad_pcb",
            vec![
                node("version", vec![Node::atom(20240108)]),
                node("generator", vec![Node::string("jenga")]),
                node(
                    "generator_version",
                    vec![Node::string(env!("CARGO_PKG_VERSION"))],
                ),
                node("general", vec![node("thickness", vec![Node::atom(1.6)])]),
                node("paper", vec![Node::string("A4")]),
                node("layers", layers),
                node(
                    "setup",
                    vec![node("pad_to_mask_clearance", vec![Node::atom(0)])],
                ),
                node("net", vec![Node::atom(0), Node::string("")]),
            ],
        );
        let mut b = Self {
            root,
            path: None,
            dirty: true,
            rules: Rules::default(),
            schematic: None,
            extra_nets: BTreeMap::new(),
            undo: vec![],
            redo: vec![],
            disk_revision: None,
            project_revision: None,
        };
        b.outline(&[
            (10., 10.),
            (10. + width, 10.),
            (10. + width, 10. + height),
            (10., 10. + height),
        ])?;
        Ok(b)
    }
    pub fn open(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("Read PCB {}", path.display()))?;
        ensure!(text.len() <= 128 * 1024 * 1024, "PCB exceeds 128 MiB");
        let root = parse(&text)?;
        ensure!(
            root.name() == "kicad_pcb",
            "Expected a modern native .kicad_pcb file"
        );
        let path = normalized_path(path);
        let meta = std::fs::read(Self::metadata_path(&path))
            .ok()
            .and_then(|s| serde_json::from_slice::<Value>(&s).ok())
            .unwrap_or(Value::Null);
        let mut rules: Rules = serde_json::from_value(meta["rules"].clone()).unwrap_or_default();
        let project_revision = std::fs::read_to_string(path.with_extension("kicad_pro"))
            .ok()
            .map(|s| revision_text(&s));
        if meta["rules"].is_null()
            || (meta["project_revision"].as_str().is_some()
                && meta["project_revision"] != json!(project_revision))
        {
            if let Ok(s) = std::fs::read(path.with_extension("kicad_pro")) {
                if let Ok(pro) = serde_json::from_slice::<Value>(&s) {
                    let r = &pro["board"]["design_settings"]["rules"];
                    if let Some(v) = r["min_clearance"].as_f64().filter(|v| *v > 0.) {
                        rules.clearance = v;
                    }
                    if let Some(v) = r["min_track_width"].as_f64().filter(|v| *v > 0.) {
                        rules.track_width = v;
                    }
                }
            }
        }
        Ok(Self {
            root,
            path: Some(path),
            dirty: false,
            rules,
            schematic: meta["schematic"].as_str().map(PathBuf::from),
            extra_nets: serde_json::from_value(meta["extra_nets"].clone()).unwrap_or_default(),
            undo: vec![],
            redo: vec![],
            disk_revision: Some(revision_text(&text)),
            project_revision,
        })
    }
    pub fn metadata_path(path: &Path) -> PathBuf {
        path.parent()
            .unwrap_or(Path::new("."))
            .join(".jenga/boards")
            .join(format!(
                "{}.json",
                path.file_stem().unwrap_or_default().to_string_lossy()
            ))
    }
    pub fn revision(&self) -> String {
        revision_text(&format!(
            "{}{}{}{}",
            self.root.encoded(),
            serde_json::to_string(&self.rules).unwrap(),
            self.schematic
                .as_ref()
                .map(|p| p.to_string_lossy())
                .unwrap_or_default(),
            serde_json::to_string(&self.extra_nets).unwrap()
        ))
    }
    pub fn disk_changed(&self) -> Result<bool> {
        if let (Some(path), Some(stamp)) = (&self.path, &self.disk_revision) {
            if let Some(pro_stamp) = &self.project_revision {
                if std::fs::read_to_string(path.with_extension("kicad_pro"))
                    .map(|s| revision_text(&s))
                    .ok()
                    .as_ref()
                    != Some(pro_stamp)
                {
                    return Ok(true);
                }
            }
            return Ok(!path.is_file() || revision_text(&std::fs::read_to_string(path)?) != *stamp);
        }
        Ok(false)
    }
    pub fn save(&mut self, path: &Path) -> Result<()> {
        ensure!(
            path.extension().is_some_and(|e| e == "kicad_pcb"),
            "PCB save path must end in .kicad_pcb"
        );
        if self
            .path
            .as_ref()
            .is_some_and(|p| normalized_path(p) == normalized_path(path))
        {
            ensure!(
                !self.disk_changed()?,
                "PCB changed externally; reopen or save a separate copy"
            );
        } else {
            ensure!(
                !path.exists(),
                "Save As destination already exists: {}",
                path.display()
            );
        }
        self.rules.validate()?;
        let text = self.root.document_text();
        let metadata = Self::metadata_path(path);
        if let Some(parent) = metadata.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let old = std::fs::read(path).ok();
        let pro_path = path.with_extension("kicad_pro");
        let old_pro = std::fs::read(&pro_path).ok();
        let mut pro: Value = if let Some(bytes) = &old_pro {
            serde_json::from_slice(bytes)
                .context("PCB project JSON is invalid; it was not overwritten")?
        } else {
            json!({"meta":{"filename":pro_path.file_name().unwrap_or_default().to_string_lossy(),"version":1}})
        };
        ensure!(pro.is_object(), "Expected native KiCad project object");
        if pro["meta"].is_null() {
            pro["meta"] = json!({"version":1});
        }
        pro["meta"]["filename"] = json!(pro_path.file_name().unwrap_or_default().to_string_lossy());
        if pro["board"].is_null() {
            pro["board"] = json!({});
        }
        if pro["board"]["design_settings"].is_null() {
            pro["board"]["design_settings"] = json!({});
        }
        if pro["board"]["design_settings"]["rules"].is_null() {
            pro["board"]["design_settings"]["rules"] = json!({});
        }
        let rules = &mut pro["board"]["design_settings"]["rules"];
        rules["min_clearance"] = json!(self.rules.clearance);
        rules["min_track_width"] = json!(self.rules.track_width);
        rules["min_copper_edge_clearance"] = json!(self.rules.edge_clearance);
        rules["min_via_diameter"] = json!(self.rules.via_diameter);
        rules["min_through_hole_diameter"] = json!(self.rules.via_drill);
        let pro_text = serde_json::to_string_pretty(&pro)?;
        atomic_write(path, text.as_bytes(), true)?;
        if let Err(e) = atomic_write(&pro_path, pro_text.as_bytes(), true) {
            if let Some(bytes) = &old {
                let _ = atomic_write(path, bytes, false);
            } else {
                let _ = std::fs::remove_file(path);
            }
            return Err(e);
        }
        if let Err(e) = atomic_write(
            &metadata,
            &serde_json::to_vec_pretty(
                &json!({"rules":self.rules,"schematic":self.schematic,"extra_nets":self.extra_nets,"project_revision":revision_text(&pro_text),"version":1}),
            )?,
            true,
        ) {
            if let Some(bytes) = &old_pro {
                let _ = atomic_write(&pro_path, bytes, false);
            } else {
                let _ = std::fs::remove_file(&pro_path);
            }
            if let Some(bytes) = old {
                let _ = atomic_write(path, &bytes, false);
            } else {
                let _ = std::fs::remove_file(path);
            }
            return Err(e);
        }
        self.path = Some(normalized_path(path));
        self.disk_revision = Some(revision_text(&text));
        self.project_revision = Some(revision_text(&pro_text));
        self.dirty = false;
        Ok(())
    }
    pub fn thickness(&self) -> f64 {
        self.root
            .child("general")
            .and_then(|n| n.child("thickness"))
            .map(|n| n.number(1))
            .filter(|n| *n > 0.)
            .unwrap_or(1.6)
    }
    pub fn copper_layers(&self) -> Vec<String> {
        self.root
            .child("layers")
            .map(|n| {
                n.items()
                    .iter()
                    .skip(1)
                    .filter(|n| n.value(1).ends_with(".Cu"))
                    .map(|n| n.value(1).to_owned())
                    .collect()
            })
            .unwrap_or_else(|| vec!["F.Cu".into(), "B.Cu".into()])
    }
    pub fn nets(&self) -> BTreeMap<i64, String> {
        let mut nets: BTreeMap<_, _> = self
            .root
            .children("net")
            .map(|n| (n.number(1) as i64, n.value(2).to_owned()))
            .collect();
        for pad in self
            .root
            .children("footprint")
            .flat_map(|f| f.children("pad"))
        {
            if let Some(n) = pad.child("net") {
                let code = net_code(pad);
                let name = if matches!(n.items().get(1), Some(Node::String(_))) {
                    n.value(1)
                } else {
                    n.value(2)
                };
                nets.entry(code).or_insert_with(|| name.to_owned());
            }
        }
        for n in self
            .root
            .items()
            .iter()
            .filter(|n| matches!(n.name(), "segment" | "arc" | "via" | "zone"))
        {
            if let Some(net) = n.child("net") {
                if matches!(net.items().get(1), Some(Node::String(_))) {
                    nets.insert(net_code(n), net.value(1).to_owned());
                }
            }
        }
        nets.extend(self.extra_nets.clone());
        nets.entry(0).or_default();
        nets
    }
    pub fn named_nets(&self) -> bool {
        self.root
            .child("version")
            .is_some_and(|n| n.number(1) >= 20260206.)
    }
    pub fn net_node(&self, code: i64) -> Node {
        node(
            "net",
            vec![if self.named_nets() {
                Node::string(self.nets().get(&code).cloned().unwrap_or_default())
            } else {
                Node::atom(code)
            }],
        )
    }
    pub fn ensure_net(&mut self, name: &str) -> i64 {
        if name.is_empty() {
            return 0;
        }
        if let Some((id, _)) = self.nets().iter().find(|(_, n)| n.as_str() == name) {
            return *id;
        }
        let id = self.nets().keys().copied().max().unwrap_or(0) + 1;
        if self.named_nets() {
            let id = named_net_code(name);
            self.extra_nets.insert(id, name.into());
            self.dirty = true;
            return id;
        }
        self.root
            .items_mut()
            .push(node("net", vec![Node::atom(id), Node::string(name)]));
        id
    }
    pub fn set_pad_net(&mut self, fp: usize, number: &str, net: i64) -> Result<()> {
        let name = self.nets().get(&net).cloned().context("Unknown net code")?;
        let node = if self.named_nets() {
            self.net_node(net)
        } else {
            node("net", vec![Node::atom(net), Node::string(&name)])
        };
        let mut found = false;
        for pad in self.root.items_mut()[fp]
            .items_mut()
            .iter_mut()
            .filter(|n| n.name() == "pad" && n.value(1) == number)
        {
            ensure!(
                pad.value(2) != "np_thru_hole",
                "Mechanical hole {number} cannot carry a net"
            );
            pad.set_child(node.clone());
            found = true;
        }
        ensure!(found, "Pad {number} not found");
        self.dirty = true;
        Ok(())
    }
    pub fn target(&self, v: &Value) -> Result<usize> {
        let uuid = v["uuid"].as_str();
        let reference = v["reference"].as_str();
        let matches: Vec<_> = self
            .root
            .items()
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                if let Some(uuid) = uuid {
                    id(n) == uuid
                } else {
                    n.name() == "footprint" && reference.is_some_and(|r| field(n, "Reference") == r)
                }
            })
            .map(|(i, _)| i)
            .collect();
        ensure!(
            matches.len() == 1,
            "PCB target must identify exactly one object: uuid={} reference={}",
            v["uuid"],
            v["reference"]
        );
        Ok(matches[0])
    }
    pub fn pads(&self) -> Vec<Pad> {
        let nets = self.nets();
        let mut pads = vec![];
        for (i, f) in self
            .root
            .items()
            .iter()
            .enumerate()
            .filter(|(_, n)| n.name() == "footprint")
        {
            for n in f.children("pad") {
                let net = net_code(n);
                let drill = n
                    .child("drill")
                    .map(|d| d.number(if d.value(1) == "oval" { 2 } else { 1 }))
                    .unwrap_or(0.);
                pads.push(Pad {
                    reference: field(f, "Reference").into(),
                    footprint_uuid: id(f).into(),
                    uuid: id(n).into(),
                    number: n.value(1).into(),
                    kind: n.value(2).into(),
                    shape: n.value(3).into(),
                    at: transform(point(n, "at"), point(f, "at"), angle(f)),
                    size: point(n, "size"),
                    angle: angle(n),
                    layers: n
                        .child("layers")
                        .map(|l| {
                            l.items()
                                .iter()
                                .skip(1)
                                .map(|n| n.text().to_owned())
                                .collect()
                        })
                        .unwrap_or_default(),
                    net,
                    net_name: nets.get(&net).cloned().unwrap_or_default(),
                    drill,
                    object: i,
                });
            }
        }
        pads
    }
    pub fn pad(&self, v: &Value) -> Result<Pad> {
        if let Some(uuid) = v["pad_uuid"].as_str() {
            return self
                .pads()
                .into_iter()
                .find(|p| p.uuid == uuid)
                .context("Exact pad_uuid not found");
        }
        let target = self.target(v)?;
        let number = v["number"]
            .as_str()
            .or_else(|| v["pad"].as_str())
            .context("Pin endpoint needs physical pad number")?;
        let matches: Vec<_> = self
            .pads()
            .into_iter()
            .filter(|p| p.object == target && p.number == number)
            .collect();
        ensure!(
            matches.len() == 1,
            "{} pad {} is missing or has multiple physical locations; use an exact pad UUID for stacked pads",
            v["reference"],
            number
        );
        Ok(matches[0].clone())
    }
    pub fn add_footprint(
        &mut self,
        id_name: &str,
        def: &Node,
        reference: &str,
        value: &str,
        at: Point,
        rotation: f64,
    ) -> Result<String> {
        ensure!(
            def.name() == "footprint" || def.name() == "module",
            "Expected a native footprint definition"
        );
        ensure!(
            !reference.is_empty()
                && !self
                    .root
                    .children("footprint")
                    .any(|f| field(f, "Reference") == reference),
            "Footprint reference {reference} is empty or already present"
        );
        let mut f = def.clone();
        f.items_mut()[0] = Node::atom("footprint");
        f.items_mut()[1] = Node::string(id_name);
        f.items_mut().retain(|n| {
            !matches!(
                n.name(),
                "version" | "generator" | "generator_version" | "uuid" | "tstamp" | "path"
            )
        });
        fn fresh(n: &mut Node) {
            if let Node::List(items) = n {
                for item in items.iter_mut() {
                    if matches!(item.name(), "uuid" | "tstamp") {
                        item.items_mut()[1] = Node::string(uuid::Uuid::new_v4());
                    } else {
                        fresh(item);
                    }
                }
            }
        }
        fresh(&mut f);
        f.set_child(uid());
        f.set_child(node("layer", vec![Node::string("F.Cu")]));
        f.set_child(node(
            "at",
            vec![Node::atom(at.0), Node::atom(at.1), Node::atom(rotation)],
        ));
        set_field(&mut f, "Reference", reference);
        set_field(&mut f, "Value", value);
        for child in f
            .items_mut()
            .iter_mut()
            .filter(|n| matches!(n.name(), "pad" | "property" | "fp_text"))
        {
            let p = point(child, "at");
            let orientation = angle(child) + rotation;
            child.set_child(node(
                "at",
                vec![Node::atom(p.0), Node::atom(p.1), Node::atom(orientation)],
            ));
        }
        for pad in f.items_mut().iter_mut().filter(|n| n.name() == "pad") {
            pad.items_mut().retain(|n| n.name() != "net");
            if id(pad).is_empty() {
                pad.set_child(uid());
            }
        }
        let uuid = id(&f).to_owned();
        self.root.items_mut().push(f);
        self.dirty = true;
        Ok(uuid)
    }
    pub fn outline(&mut self, points: &[Point]) -> Result<()> {
        ensure!(
            points.len() >= 3 && points.len() <= 10000,
            "Outline needs 3–10000 vertices"
        );
        ensure!(
            points.iter().all(|p| p.0.is_finite() && p.1.is_finite()),
            "Nonfinite board outline"
        );
        self.root.items_mut().retain(|n| {
            !(matches!(n.name(), "gr_line" | "gr_rect" | "gr_arc" | "gr_poly")
                && layer(n) == "Edge.Cuts")
        });
        for (i, p) in points.iter().enumerate() {
            let q = points[(i + 1) % points.len()];
            ensure!(*p != q, "Zero-length board outline edge");
            self.root.items_mut().push(node(
                "gr_line",
                vec![
                    xy("start", *p),
                    xy("end", q),
                    node(
                        "stroke",
                        vec![
                            node("width", vec![Node::atom(0.05)]),
                            node("type", vec![Node::atom("default")]),
                        ],
                    ),
                    node("layer", vec![Node::string("Edge.Cuts")]),
                    uid(),
                ],
            ));
        }
        self.dirty = true;
        Ok(())
    }
    pub fn outline_segments(&self) -> Vec<(Point, Point)> {
        let mut segments = vec![];
        for n in self.root.items().iter().filter(|n| layer(n) == "Edge.Cuts") {
            match n.name() {
                "gr_line" => segments.push((point(n, "start"), point(n, "end"))),
                "gr_rect" => {
                    let a = point(n, "start");
                    let b = point(n, "end");
                    let p = [a, (b.0, a.1), b, (a.0, b.1)];
                    for i in 0..4 {
                        segments.push((p[i], p[(i + 1) % 4]));
                    }
                }
                "gr_poly" => {
                    let p = pts(n);
                    for i in 0..p.len() {
                        segments.push((p[i], p[(i + 1) % p.len()]));
                    }
                }
                "gr_arc" => {
                    let p = arc_points(n);
                    for w in p.windows(2) {
                        segments.push((w[0], w[1]));
                    }
                }
                _ => {}
            }
        }
        segments
    }
    pub fn bounds(&self) -> (Point, Point) {
        let p: Vec<_> = self
            .outline_segments()
            .iter()
            .flat_map(|(a, b)| [*a, *b])
            .collect();
        if p.is_empty() {
            return ((0., 0.), (100., 80.));
        }
        crate::scene::bounds(&p)
    }
    pub fn inside(&self, p: Point) -> bool {
        let edges = self.outline_segments();
        if edges.is_empty() {
            return false;
        }
        let mut count = 0;
        for (a, b) in edges {
            if crate::pcb_checks::point_segment(p, a, b) < 1e-6 {
                return true;
            }
            if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
                count += 1;
            }
        }
        count % 2 == 1
    }
    pub fn segment(
        &mut self,
        a: Point,
        b: Point,
        width: f64,
        layer_name: &str,
        net: i64,
    ) -> Result<String> {
        ensure!(a != b, "Zero-length PCB track");
        ensure!(
            self.copper_layers().iter().any(|l| l == layer_name),
            "Copper layer {layer_name} is absent from this stackup"
        );
        ensure!(
            width.is_finite() && width > 0. && width < 100.,
            "Track width must be positive millimetres"
        );
        ensure!(
            self.nets().contains_key(&net) && net > 0,
            "Track requires an assigned net"
        );
        let n = node(
            "segment",
            vec![
                xy("start", a),
                xy("end", b),
                node("width", vec![Node::atom(width)]),
                node("layer", vec![Node::string(layer_name)]),
                self.net_node(net),
                uid(),
            ],
        );
        let uuid = id(&n).to_owned();
        self.root.items_mut().push(n);
        self.dirty = true;
        Ok(uuid)
    }
    pub fn via(&mut self, at: Point, net: i64) -> Result<String> {
        self.rules.validate()?;
        ensure!(
            net > 0 && self.nets().contains_key(&net),
            "Via requires an assigned net"
        );
        let n = node(
            "via",
            vec![
                xy("at", at),
                node("size", vec![Node::atom(self.rules.via_diameter)]),
                node("drill", vec![Node::atom(self.rules.via_drill)]),
                node("layers", vec![Node::string("F.Cu"), Node::string("B.Cu")]),
                self.net_node(net),
                uid(),
            ],
        );
        let uuid = id(&n).to_owned();
        self.root.items_mut().push(n);
        self.dirty = true;
        Ok(uuid)
    }
    pub fn inspect(&self, p: &Value) -> Value {
        let pads = self.pads();
        let refs = p["references"].as_array();
        let ids = p["uuids"].as_array();
        let kinds = p["kinds"].as_array();
        let objects:Vec<_>=self.root.items().iter().enumerate().filter(|(_,n)|matches!(n.name(),"footprint"|"segment"|"arc"|"via"|"zone"|"gr_line"|"gr_arc"|"gr_rect"|"gr_poly"|"gr_text"|"dimension"|"target"|"image")).filter(|(_,n)|refs.is_none_or(|r|r.contains(&json!(field(n,"Reference"))))&&ids.is_none_or(|r|r.contains(&json!(id(n))))&&kinds.is_none_or(|r|r.contains(&json!(n.name())))).map(|(i,n)|json!({"uuid":id(n),"kind":n.name(),"reference":field(n,"Reference"),"value":field(n,"Value"),"library_id":if n.name()=="footprint"{n.value(1)}else{""},"at_mm":point(n,"at"),"angle":angle(n),"layer":layer(n),"start":point(n,"start"),"end":point(n,"end"),"width_mm":n.child("width").map(|n|n.number(1)),"net":net_code(n),"pads":if p["include_pads"]==false{json!(null)}else{json!(pads.iter().filter(|p|p.object==i).collect::<Vec<_>>())},"models":n.children("model").map(|m|json!({"path":m.value(1),"offset":xyz(m,"offset",[0.,0.,0.]),"scale":xyz(m,"scale",[1.,1.,1.]),"rotation":xyz(m,"rotate",[0.,0.,0.])})).collect::<Vec<_>>() })).collect();
        json!({"file":self.path,"revision":self.revision(),"dirty":self.dirty,"kind":"pcb","schematic":self.schematic,"layers":self.copper_layers(),"thickness_mm":self.thickness(),"bounds_mm":self.bounds(),"rules":self.rules,"nets":self.nets().iter().map(|(n,name)|json!({"code":n,"name":name})).collect::<Vec<_>>(),"objects":objects,"warnings":self.warnings()})
    }
    pub fn warnings(&self) -> Vec<String> {
        let mut result = vec![];
        if self
            .root
            .child("version")
            .is_some_and(|v| v.number(1) > 20260206.)
        {
            result.push("Newer PCB format: unfamiliar fields are preserved, but full future-format rendering is not certified".into());
        }
        for kind in ["group", "dimension", "image", "table", "generator"] {
            if self.root.children(kind).next().is_some() && kind != "generator" {
                result.push(format!(
                    "{kind} data is preserved; advanced editing is restricted"
                ));
            }
        }
        result
    }
    pub fn scene(&self, visible: Option<&[String]>, ratsnest: bool) -> Scene {
        let mut s = Scene::default();
        let show = |l: &str| visible.is_none_or(|v| v.iter().any(|x| x == l || x == "*.Cu"));
        let color = |l: &str| {
            if l == "F.Cu" {
                [190, 35, 35]
            } else if l == "B.Cu" {
                [35, 75, 195]
            } else if l == "Edge.Cuts" {
                [35, 35, 40]
            } else if l.ends_with("SilkS") {
                [45, 70, 60]
            } else {
                [135, 135, 140]
            }
        };
        for (i, n) in self.root.items().iter().enumerate() {
            match n.name() {
                "footprint" => {
                    let mut f = n.clone();
                    if let Some(v) = visible {
                        f.items_mut().retain(|child| {
                            if child.name() == "pad" {
                                if child.value(2) == "np_thru_hole" {
                                    return show("Edge.Cuts");
                                }
                                return child.child("layers").is_some_and(|l| {
                                    l.items().iter().skip(1).any(|l| {
                                        l.text().ends_with(".Cu")
                                            && (l.text() == "*.Cu"
                                                && v.iter().any(|n| n.ends_with(".Cu"))
                                                || show(l.text()))
                                    })
                                });
                            }
                            if matches!(
                                child.name(),
                                "property"
                                    | "fp_text"
                                    | "fp_line"
                                    | "fp_rect"
                                    | "fp_poly"
                                    | "fp_arc"
                                    | "fp_circle"
                            ) {
                                return show(layer(child));
                            }
                            true
                        });
                    }
                    for child in f
                        .items_mut()
                        .iter_mut()
                        .filter(|n| matches!(n.name(), "pad" | "property" | "fp_text"))
                    {
                        let p = point(child, "at");
                        let local = angle(child) - angle(n);
                        child.set_child(node(
                            "at",
                            vec![Node::atom(p.0), Node::atom(p.1), Node::atom(local)],
                        ));
                    }
                    for p in f.items_mut().iter_mut().filter(|p| p.name() == "property") {
                        if p.value(1) == "Reference" || p.value(1) == "Value" {
                            p.items_mut()[0] = Node::atom("fp_text");
                            p.items_mut()[1] = Node::atom(p.value(1).to_lowercase());
                        }
                    }
                    for text in f.items_mut().iter_mut().filter(|p| p.name() == "fp_text") {
                        if text.value(2) == "${REFERENCE}" {
                            text.items_mut()[2] = Node::string(field(n, "Reference"));
                        }
                    }
                    let fs = Scene::footprint_preview(&f);
                    let at = point(n, "at");
                    let r = angle(n);
                    for d in fs.draws {
                        let mut shape = d.shape;
                        map_shape(&mut shape, |p| transform(p, at, r), r);
                        s.draws.push(Draw {
                            shape,
                            object: Some(i),
                        });
                    }
                }
                "segment" => {
                    if show(layer(n)) {
                        s.draws.push(Draw {
                            shape: Shape::Path {
                                points: vec![point(n, "start"), point(n, "end")],
                                closed: false,
                                fill: None,
                                color: color(layer(n)),
                                width: n.child("width").map(|n| n.number(1)).unwrap_or(0.25),
                            },
                            object: Some(i),
                        });
                    }
                }
                "via" => {
                    s.draws.push(Draw {
                        shape: Shape::Circle {
                            center: point(n, "at"),
                            radius: n.child("size").map(|n| n.number(1) / 2.).unwrap_or(0.3),
                            fill: Some([170, 150, 50]),
                            color: [90, 90, 90],
                            width: 0.05,
                        },
                        object: Some(i),
                    });
                    s.draws.push(Draw {
                        shape: Shape::Circle {
                            center: point(n, "at"),
                            radius: n.child("drill").map(|n| n.number(1) / 2.).unwrap_or(0.15),
                            fill: Some([248, 247, 242]),
                            color: [90, 90, 90],
                            width: 0.04,
                        },
                        object: Some(i),
                    });
                }
                "zone" => {
                    if show(layer(n)) {
                        let polygons: Vec<_> = n
                            .children("filled_polygon")
                            .chain(n.children("polygon"))
                            .collect();
                        for poly in polygons {
                            s.draws.push(Draw {
                                shape: Shape::Path {
                                    points: pts(poly),
                                    closed: true,
                                    fill: if poly.name() == "filled_polygon" {
                                        Some(if layer(n) == "F.Cu" {
                                            [235, 177, 171]
                                        } else {
                                            [177, 190, 235]
                                        })
                                    } else {
                                        None
                                    },
                                    color: color(layer(n)),
                                    width: 0.08,
                                },
                                object: Some(i),
                            });
                        }
                    }
                }
                "gr_line" | "gr_rect" | "gr_poly" | "gr_arc" | "arc" => {
                    if show(layer(n)) {
                        let p = match n.name() {
                            "gr_line" => vec![point(n, "start"), point(n, "end")],
                            "gr_rect" => {
                                let a = point(n, "start");
                                let b = point(n, "end");
                                vec![a, (b.0, a.1), b, (a.0, b.1)]
                            }
                            "gr_poly" => pts(n),
                            _ => arc_points(n),
                        };
                        let width = n
                            .child("width")
                            .or_else(|| n.child("stroke").and_then(|n| n.child("width")))
                            .map(|n| n.number(1))
                            .unwrap_or(0.1);
                        s.draws.push(Draw {
                            shape: Shape::Path {
                                points: p,
                                closed: matches!(n.name(), "gr_rect" | "gr_poly"),
                                fill: None,
                                color: color(layer(n)),
                                width,
                            },
                            object: Some(i),
                        });
                    }
                }
                "gr_text" => {
                    if show(layer(n)) {
                        s.draws.push(Draw {
                            shape: Shape::Text {
                                at: point(n, "at"),
                                text: n.value(1).into(),
                                size: n
                                    .child("effects")
                                    .and_then(|n| n.child("font"))
                                    .and_then(|n| n.child("size"))
                                    .map(|n| n.number(1))
                                    .unwrap_or(1.),
                                angle: angle(n),
                                color: color(layer(n)),
                                left: false,
                            },
                            object: Some(i),
                        });
                    }
                }
                _ => {}
            }
        }
        if ratsnest {
            for pair in crate::pcb_checks::ratsnest(self) {
                s.draws.push(Draw {
                    shape: Shape::Path {
                        points: vec![pair.0.at, pair.1.at],
                        closed: false,
                        fill: None,
                        color: [100, 165, 95],
                        width: 0.05,
                    },
                    object: None,
                });
            }
        }
        // Zones form the background. Never paint their fill over pads, tracks
        // and component outlines, regardless of native file object order.
        s.draws.sort_by_key(|d| {
            d.object
                .and_then(|i| self.root.items().get(i))
                .map_or(3, |n| match n.name() {
                    "zone" => 0,
                    "segment" | "arc" => 1,
                    "footprint" | "via" => 2,
                    _ => 3,
                })
        });
        s
    }
}
pub fn xyz(n: &Node, key: &str, default: [f64; 3]) -> [f64; 3] {
    n.child(key)
        .and_then(|n| n.child("xyz"))
        .map(|p| [p.number(1), p.number(2), p.number(3)])
        .unwrap_or(default)
}
pub fn map_shape(shape: &mut Shape, f: impl Fn(Point) -> Point, angle: f64) {
    match shape {
        Shape::Path { points, .. } => {
            for p in points {
                *p = f(*p);
            }
        }
        Shape::Circle { center, .. } => *center = f(*center),
        Shape::Text { at, angle: a, .. } => {
            *at = f(*at);
            *a += angle;
        }
    }
}
pub fn arc_points(n: &Node) -> Vec<Point> {
    let a = point(n, "start");
    let m = point(n, "mid");
    let b = point(n, "end");
    let d = 2. * (a.0 * (m.1 - b.1) + m.0 * (b.1 - a.1) + b.0 * (a.1 - m.1));
    if d.abs() < 1e-10 {
        return vec![a, b];
    }
    let aa = a.0 * a.0 + a.1 * a.1;
    let mm = m.0 * m.0 + m.1 * m.1;
    let bb = b.0 * b.0 + b.1 * b.1;
    let c = (
        (aa * (m.1 - b.1) + mm * (b.1 - a.1) + bb * (a.1 - m.1)) / d,
        (aa * (b.0 - m.0) + mm * (a.0 - b.0) + bb * (m.0 - a.0)) / d,
    );
    let r = (a.0 - c.0).hypot(a.1 - c.1);
    let start = (a.1 - c.1).atan2(a.0 - c.0);
    let mid = (m.1 - c.1).atan2(m.0 - c.0);
    let end = (b.1 - c.1).atan2(b.0 - c.0);
    let tau = std::f64::consts::TAU;
    let mut span = (end - start).rem_euclid(tau);
    if (mid - start).rem_euclid(tau) > span {
        span -= tau;
    }
    let count = (r * span.abs() / 0.25).ceil().clamp(4., 256.) as usize;
    (0..=count)
        .map(|i| {
            let t = start + span * i as f64 / count as f64;
            (c.0 + r * t.cos(), c.1 + r * t.sin())
        })
        .collect()
}

pub fn footprint_source(p: &Value) -> Result<Node> {
    let name = p["name"].as_str().context("Footprint needs name")?;
    ensure!(
        !name.is_empty() && !name.contains(['/', '\\', ':', '"']),
        "Use a simple footprint name"
    );
    if let Some(source) = p["native_source"].as_str() {
        ensure!(
            source.len() < 16 * 1024 * 1024,
            "Footprint source exceeds 16 MiB"
        );
        let mut n = parse(source)?;
        ensure!(
            matches!(n.name(), "footprint" | "module"),
            "Expected one native footprint"
        );
        n.items_mut()[0] = Node::atom("footprint");
        n.items_mut()[1] = Node::string(name);
        validate_footprint(&n)?;
        return Ok(n);
    }
    let pads = p["pads"]
        .as_array()
        .context("Footprint needs pads or native_source")?;
    ensure!(
        !pads.is_empty() && pads.len() <= 10000,
        "Footprint needs 1–10000 pads"
    );
    let mut n = node(
        "footprint",
        vec![
            Node::string(name),
            node("version", vec![Node::atom(20240108)]),
            node("generator", vec![Node::string("jenga")]),
            node("layer", vec![Node::string("F.Cu")]),
            node(
                "attr",
                vec![Node::atom(
                    if pads.iter().any(|p| p["kind"] == "thru_hole") {
                        "through_hole"
                    } else {
                        "smd"
                    },
                )],
            ),
        ],
    );
    set_field(&mut n, "Reference", "REF**");
    set_field(&mut n, "Value", name);
    for p in pads {
        let number = p["number"].as_str().unwrap_or("");
        let kind = p["kind"].as_str().unwrap_or("smd");
        let shape = p["shape"].as_str().unwrap_or("rect");
        ensure!(
            matches!(kind, "smd" | "thru_hole" | "np_thru_hole" | "connect"),
            "Invalid pad type {kind}"
        );
        ensure!(
            matches!(
                shape,
                "rect" | "circle" | "oval" | "roundrect" | "trapezoid" | "custom"
            ),
            "Invalid pad shape {shape}"
        );
        let at = json_point(p, "at")?;
        let size = json_point(p, "size")?;
        let layers = p["layers"].as_array().cloned().unwrap_or_else(|| {
            if kind == "smd" {
                vec![json!("F.Cu"), json!("F.Paste"), json!("F.Mask")]
            } else {
                vec![json!("*.Cu"), json!("*.Mask")]
            }
        });
        let mut pad = node(
            "pad",
            vec![
                Node::string(number),
                Node::atom(kind),
                Node::atom(shape),
                node(
                    "at",
                    vec![
                        Node::atom(at.0),
                        Node::atom(at.1),
                        Node::atom(p["rotation"].as_f64().unwrap_or(0.)),
                    ],
                ),
                xy("size", size),
                node(
                    "layers",
                    layers
                        .iter()
                        .map(|n| Node::string(n.as_str().unwrap_or("")))
                        .collect(),
                ),
                uid(),
            ],
        );
        if kind == "thru_hole" || kind == "np_thru_hole" {
            pad.set_child(node(
                "drill",
                vec![Node::atom(
                    p["drill_mm"]
                        .as_f64()
                        .context("Through-hole pad requires drill_mm")?,
                )],
            ));
        }
        if shape == "roundrect" {
            pad.set_child(node(
                "roundrect_rratio",
                vec![Node::atom(p["roundrect_ratio"].as_f64().unwrap_or(0.25))],
            ));
        }
        ensure!(
            shape != "custom",
            "Author custom pad primitives through native_source"
        );
        n.items_mut().push(pad);
    }
    for g in p["graphics"].as_array().into_iter().flatten() {
        let kind = g["kind"].as_str().unwrap_or("line");
        let name = match kind {
            "line" => "fp_line",
            "rectangle" => "fp_rect",
            "circle" => "fp_circle",
            "arc" => "fp_arc",
            "polygon" => "fp_poly",
            "text" => "fp_text",
            _ => anyhow::bail!("Unknown footprint graphic {kind}"),
        };
        let mut gnode = node(name, vec![]);
        if kind == "text" {
            gnode.items_mut().extend([
                Node::atom("user"),
                Node::string(g["text"].as_str().context("Text graphic needs text")?),
                node(
                    "at",
                    vec![
                        Node::atom(json_point(g, "at")?.0),
                        Node::atom(json_point(g, "at")?.1),
                        Node::atom(0),
                    ],
                ),
                node(
                    "effects",
                    vec![node(
                        "font",
                        vec![
                            xy("size", (1., 1.)),
                            node("thickness", vec![Node::atom(0.15)]),
                        ],
                    )],
                ),
            ]);
        } else if kind == "polygon" {
            let ps = g["points"].as_array().context("Polygon needs points")?;
            gnode.set_child(node(
                "pts",
                ps.iter()
                    .map(|p| json_point(&json!({"p":p}), "p").map(|p| xy("xy", p)))
                    .collect::<Result<Vec<_>>>()?,
            ));
        } else {
            gnode.set_child(xy(
                if kind == "circle" { "center" } else { "start" },
                json_point(g, if kind == "circle" { "center" } else { "start" })?,
            ));
            gnode.set_child(xy("end", json_point(g, "end")?));
            if kind == "arc" {
                gnode.set_child(xy("mid", json_point(g, "mid")?));
            }
        }
        gnode.set_child(node(
            "layer",
            vec![Node::string(g["layer"].as_str().unwrap_or("F.SilkS"))],
        ));
        if kind != "text" {
            gnode.set_child(node(
                "stroke",
                vec![
                    node(
                        "width",
                        vec![Node::atom(g["width_mm"].as_f64().unwrap_or(0.15))],
                    ),
                    node("type", vec![Node::atom("default")]),
                ],
            ));
        }
        gnode.set_child(uid());
        n.items_mut().push(gnode);
    }
    for m in p["models"].as_array().into_iter().flatten() {
        let mut model = node(
            "model",
            vec![Node::string(
                m["path"].as_str().context("Model needs path")?,
            )],
        );
        for (key, default) in [
            ("offset", [0., 0., 0.]),
            ("scale", [1., 1., 1.]),
            ("rotate", [0., 0., 0.]),
        ] {
            let values = m[key]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|n| n.as_f64().unwrap_or(f64::NAN))
                        .collect::<Vec<_>>()
                })
                .unwrap_or(default.to_vec());
            ensure!(
                values.len() == 3 && values.iter().all(|v| v.is_finite()),
                "3D {key} requires three finite coordinates"
            );
            model.set_child(node(
                key,
                vec![node("xyz", values.iter().map(Node::atom).collect())],
            ));
        }
        n.items_mut().push(model);
    }
    validate_footprint(&n)?;
    Ok(n)
}
pub fn json_point(p: &Value, key: &str) -> Result<Point> {
    let a = p[key]
        .as_array()
        .with_context(|| format!("{key} requires [x,y] millimetres"))?;
    ensure!(a.len() == 2, "{key} needs two coordinates");
    let x = a[0].as_f64().context("Invalid x")?;
    let y = a[1].as_f64().context("Invalid y")?;
    ensure!(
        x.is_finite() && y.is_finite() && x.abs() < 100000. && y.abs() < 100000.,
        "Invalid {key} coordinates"
    );
    Ok((x, y))
}
pub fn validate_footprint(n: &Node) -> Result<()> {
    let mut numbers = std::collections::HashSet::new();
    for p in n.children("pad") {
        let size = point(p, "size");
        ensure!(
            size.0 > 0. && size.1 > 0. && size.0.is_finite() && size.1.is_finite(),
            "Pad {} has invalid size",
            p.value(1)
        );
        if p.value(2) != "np_thru_hole" {
            ensure!(
                !p.value(1).is_empty(),
                "Electrical pad needs a physical number"
            );
            numbers.insert(p.value(1));
        }
        if let Some(d) = p.child("drill") {
            let drill = d.number(if d.value(1) == "oval" { 2 } else { 1 });
            ensure!(
                drill > 0. && drill <= size.0.min(size.1),
                "Pad {} drill exceeds pad size",
                p.value(1)
            );
        }
    }
    ensure!(
        !numbers.is_empty() || n.children("pad").next().is_some(),
        "Footprint needs electrical pads or mechanical holes"
    );
    Ok(())
}
