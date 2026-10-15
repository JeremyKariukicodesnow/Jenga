use crate::sexpr::{Node, parse};
use anyhow::{Context, Result, bail};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub fn id() -> Node {
    Node::list("uuid", vec![Node::string(Uuid::new_v4())])
}
/// Ordinary absolute paths keep Windows extended-prefix aliases out of UI and IPC.
pub fn normalized_path(path: &Path) -> PathBuf {
    let full = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    #[cfg(windows)]
    {
        let text = full.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    full
}
pub fn at(x: f64, y: f64, angle: f64) -> Node {
    Node::list("at", vec![Node::atom(x), Node::atom(y), Node::atom(angle)])
}
pub fn xy(x: f64, y: f64) -> Node {
    Node::list("xy", vec![Node::atom(x), Node::atom(y)])
}
pub fn effects(size: f64) -> Node {
    Node::list(
        "effects",
        vec![Node::list(
            "font",
            vec![Node::list("size", vec![Node::atom(size), Node::atom(size)])],
        )],
    )
}
pub fn position(n: &Node) -> (f64, f64, f64) {
    n.child("at")
        .map(|a| (a.number(1), a.number(2), a.number(3)))
        .unwrap_or_default()
}

/// Original root is retained so edits never reconstruct (and discard) unknown fields.
#[derive(Clone)]
pub struct Document {
    /// Runtime page context; never serialized into KiCad source.
    pub page_context: Option<(usize, usize)>,
    pub root: Node,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub disk_stamp: Option<u64>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            page_context: None,
            root: Node::list(
                "kicad_sch",
                vec![
                    Node::list("version", vec![Node::atom(20250114)]),
                    Node::list("generator", vec![Node::string("jenga")]),
                    Node::list(
                        "generator_version",
                        vec![Node::string(env!("CARGO_PKG_VERSION"))],
                    ),
                    id(),
                    Node::list("paper", vec![Node::string("A4")]),
                    Node::list("lib_symbols", vec![]),
                    Node::list(
                        "sheet_instances",
                        vec![Node::list(
                            "path",
                            vec![
                                Node::string("/"),
                                Node::list("page", vec![Node::string("1")]),
                            ],
                        )],
                    ),
                ],
            ),
            path: None,
            dirty: false,
            disk_stamp: None,
        }
    }
    pub fn open(path: &Path) -> Result<Self> {
        let source =
            fs::read_to_string(path).with_context(|| format!("Read {}", path.display()))?;
        if source.starts_with("EESchema Schematic File") {
            bail!(
                "Legacy .sch files are not editable in Jenga. Open and save this file as .kicad_sch in KiCad first. Your original file has not been changed."
            );
        }
        let root = parse(&source)?;
        if root.name() != "kicad_sch" {
            bail!("This is not a KiCad schematic.");
        }
        if root.child("uuid").is_none() {
            bail!("Schematic has no root UUID.");
        }
        Ok(Self {
            page_context: None,
            root,
            path: Some(normalized_path(path)),
            dirty: false,
            disk_stamp: Some(Self::fingerprint(source.as_bytes())),
        })
    }
    pub fn save(&mut self, path: &Path) -> Result<()> {
        if self
            .path
            .as_ref()
            .is_some_and(|p| *p == normalized_path(path))
            && self.disk_changed()?
        {
            bail!(
                "File changed outside this editing session. Reload it or Save as a separate copy; existing edits were preserved."
            );
        }
        self.normalize_embedded_names();
        let source = self.root.document_text();
        atomic_write(path, source.as_bytes(), true)?;
        self.disk_stamp = Some(Self::fingerprint(source.as_bytes()));
        self.path = Some(normalized_path(path));
        self.dirty = false;
        Ok(())
    }
    fn fingerprint(bytes: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hash);
        hash.finish()
    }
    pub fn disk_changed(&self) -> Result<bool> {
        if let (Some(path), Some(stamp)) = (&self.path, self.disk_stamp) {
            return Ok(!path.is_file() || Self::fingerprint(&fs::read(path)?) != stamp);
        }
        Ok(false)
    }
    /// Repair old Jenga embeddings as well as newly placed inherited symbols.
    pub fn normalize_embedded_names(&mut self) {
        if let Some(libs) = self.root.child_mut("lib_symbols") {
            for def in libs.items_mut().iter_mut().filter(|n| n.name() == "symbol") {
                let name = def.value(1).rsplit(':').next().unwrap_or("").to_owned();
                for body in def.items_mut().iter_mut().filter(|n| n.name() == "symbol") {
                    let old = body.value(1).to_owned();
                    if let Some((prefix, style)) = old.rsplit_once('_')
                        && let Some((_, unit)) = prefix.rsplit_once('_')
                    {
                        body.items_mut()[1] = Node::string(format!("{name}_{unit}_{style}"));
                    }
                }
            }
        }
    }
    pub fn revision(&self) -> String {
        let mut h = 0xcbf29ce484222325u64;
        for b in self.root.encoded().bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        format!("{h:016x}")
    }
    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled schematic".into())
    }
    pub fn project_name(&self) -> String {
        if let Some(path) = &self.path
            && let Some(parent) = path.parent()
        {
            if path.with_extension("kicad_pro").exists() {
                return self.title();
            }
            if let Ok(entries) = fs::read_dir(parent) {
                let projects: Vec<_> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|e| e == "kicad_pro"))
                    .collect();
                if projects.len() == 1 {
                    return projects[0]
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                }
            }
        }
        self.title()
    }
    pub fn root_uuid(&self) -> String {
        self.root
            .child("uuid")
            .map(|n| n.value(1).to_owned())
            .unwrap_or_default()
    }
    pub fn embedded(&self, lib_id: &str) -> Option<&Node> {
        self.root
            .child("lib_symbols")?
            .children("symbol")
            .find(|s| s.value(1) == lib_id)
    }
    pub fn instance_definition(&self, instance: &Node) -> Option<&Node> {
        if let Some(alias) = instance.child("lib_name")
            && let Some(def) = self.embedded(alias.value(1))
        {
            return Some(def);
        }
        self.embedded(instance.child("lib_id")?.value(1))
    }
    pub fn add_symbol(
        &mut self,
        lib_id: &str,
        definition: &Node,
        x: f64,
        y: f64,
        unit: u32,
    ) -> usize {
        if self.embedded(lib_id).is_none() {
            let mut def = definition.clone();
            def.items_mut()[1] = Node::string(lib_id);
            self.root
                .child_mut("lib_symbols")
                .expect("lib_symbols")
                .items_mut()
                .push(def);
        }
        let prefix = definition
            .children("property")
            .find(|p| p.value(1) == "Reference")
            .map(|p| p.value(2).trim_end_matches('?'))
            .unwrap_or("U");
        let mut index = 1;
        while self
            .root
            .children("symbol")
            .any(|s| property(s, "Reference") == format!("{prefix}{index}"))
        {
            index += 1;
        }
        let reference = if unit > 1 {
            self.root
                .children("symbol")
                .filter(|n| n.child("lib_id").is_some_and(|id| id.value(1) == lib_id))
                .map(|n| property(n, "Reference"))
                .filter(|reference| {
                    !self.root.children("symbol").any(|n| {
                        property(n, "Reference") == *reference
                            && n.child("unit").is_some_and(|u| u.number(1) == unit as f64)
                    })
                })
                .last()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{prefix}{index}"))
        } else {
            format!("{prefix}{index}")
        };
        let (body_a, body_b) = crate::scene::Scene::symbol_body_bounds(definition, unit);
        let mut properties = vec![];
        for prop in definition
            .children("property")
            .filter(|p| !p.value(1).starts_with("ki_"))
        {
            let mut p = prop.clone();
            if p.value(1) == "Reference" {
                p.items_mut()[2] = Node::string(&reference);
            }
            let (px, py, pa) = position(prop);
            p.set_child(at(x + px, y - py, pa));
            // KiCad library field positions are defaults; instance fields need auto-placement.
            if matches!(p.value(1), "Reference" | "Value")
                && definition.children("symbol").next().is_some()
                && !p
                    .child("do_not_autoplace")
                    .is_some_and(|n| n.value(1) == "yes")
            {
                let reference = p.value(1) == "Reference";
                if body_b.0 - body_a.0 < 10. && body_b.1 - body_a.1 > body_b.0 - body_a.0 {
                    p.set_child(at(
                        x + body_b.0 + 1.5,
                        y + if reference { -1.1 } else { 1.1 },
                        0.,
                    ));
                    if let Some(e) = p.child_mut("effects") {
                        e.set_child(Node::list("justify", vec![Node::atom("left")]));
                    }
                } else {
                    p.set_child(at(
                        x + (body_a.0 + body_b.0) / 2.,
                        y + if reference {
                            body_a.1 - 2.0
                        } else {
                            body_b.1 + 2.0
                        },
                        0.,
                    ));
                    if let Some(e) = p.child_mut("effects") {
                        e.items_mut().retain(|n| n.name() != "justify");
                    }
                }
            }
            properties.push(p);
        }
        let mut data = vec![
            Node::list("lib_id", vec![Node::string(lib_id)]),
            at(x, y, 0.0),
            Node::list("unit", vec![Node::atom(unit)]),
            Node::list("in_bom", vec![Node::atom("yes")]),
            Node::list("on_board", vec![Node::atom("yes")]),
            Node::list("dnp", vec![Node::atom("no")]),
            id(),
        ];
        data.extend(properties);
        data.push(Node::list(
            "instances",
            vec![Node::list(
                "project",
                vec![
                    Node::string(self.project_name()),
                    Node::list(
                        "path",
                        vec![
                            Node::string(format!("/{}", self.root_uuid())),
                            Node::list("reference", vec![Node::string(reference)]),
                            Node::list("unit", vec![Node::atom(unit)]),
                        ],
                    ),
                ],
            )],
        ));
        self.root.items_mut().push(Node::list("symbol", data));
        self.dirty = true;
        self.root.items().len() - 1
    }
    pub fn add_wire(&mut self, a: (f64, f64), b: (f64, f64)) {
        if a == b {
            return;
        }
        self.root.items_mut().push(Node::list(
            "wire",
            vec![
                Node::list("pts", vec![xy(a.0, a.1), xy(b.0, b.1)]),
                Node::list(
                    "stroke",
                    vec![
                        Node::list("width", vec![Node::atom(0)]),
                        Node::list("type", vec![Node::atom("default")]),
                    ],
                ),
                id(),
            ],
        ));
        self.dirty = true;
    }
    pub fn add_text(&mut self, name: &str, text: &str, x: f64, y: f64) -> usize {
        let mut data = vec![Node::string(text), at(x, y, 0.0), effects(1.27), id()];
        if matches!(name, "global_label" | "hierarchical_label") {
            data.push(Node::list("shape", vec![Node::atom("input")]));
        }
        if name == "label" {
            data[2].items_mut().push(Node::list(
                "justify",
                vec![Node::atom("left"), Node::atom("bottom")],
            ));
        }
        self.root.items_mut().push(Node::list(name, data));
        self.dirty = true;
        self.root.items().len() - 1
    }
    pub fn add_point(&mut self, kind: &str, x: f64, y: f64) {
        let mut data = vec![Node::list("at", vec![Node::atom(x), Node::atom(y)]), id()];
        if kind == "junction" {
            data.extend([
                Node::list("diameter", vec![Node::atom(0)]),
                Node::list("color", vec![Node::atom(0); 4]),
            ]);
        }
        self.root.items_mut().push(Node::list(kind, data));
        self.dirty = true;
    }
    pub fn translate(&mut self, index: usize, dx: f64, dy: f64) {
        if let Some(n) = self.root.items_mut().get_mut(index) {
            if n.child("at").is_some() {
                let (x, y, a) = position(n);
                if matches!(n.name(), "junction" | "no_connect") {
                    n.set_child(Node::list(
                        "at",
                        vec![Node::atom(x + dx), Node::atom(y + dy)],
                    ));
                } else {
                    n.set_child(at(x + dx, y + dy, a));
                }
            }
            for p in n.items_mut().iter_mut().filter(|p| p.name() == "property") {
                let (x, y, a) = position(p);
                p.set_child(at(x + dx, y + dy, a));
            }
            if let Some(pts) = n.child_mut("pts") {
                for p in pts.items_mut().iter_mut().skip(1) {
                    let (x, y) = (p.number(1), p.number(2));
                    *p = xy(x + dx, y + dy);
                }
            }
            self.dirty = true;
        }
    }
    pub fn rotate(&mut self, index: usize) {
        if let Some(n) = self.root.items_mut().get_mut(index) {
            if n.name() != "symbol" {
                return;
            }
            let (x, y, a) = position(n);
            n.set_child(at(x, y, (a + 90.0) % 360.0));
            for p in n.items_mut().iter_mut().filter(|p| p.name() == "property") {
                let (px, py, pa) = position(p);
                let mut angle = (pa + 90.0).rem_euclid(360.0);
                if ["Reference", "Value"].contains(&p.value(1)) && angle > 90. && angle <= 270. {
                    angle = (angle + 180.).rem_euclid(360.);
                }
                p.set_child(at(x + py - y, y - (px - x), angle));
            }
            self.dirty = true;
        }
    }
    pub fn set_property(&mut self, index: usize, key: &str, value: &str) {
        if let Some(n) = self.root.items_mut().get_mut(index) {
            if let Some(p) = n
                .items_mut()
                .iter_mut()
                .find(|p| p.name() == "property" && p.value(1) == key)
            {
                p.items_mut()[2] = Node::string(value);
            } else {
                let (x, y, _) = position(n);
                n.items_mut().push(Node::list(
                    "property",
                    vec![
                        Node::string(key),
                        Node::string(value),
                        at(x, y, 0.),
                        Node::list("hide", vec![Node::atom("yes")]),
                        effects(1.27),
                    ],
                ));
            }
            if key == "Reference" {
                update_reference(n, value);
            }
            self.dirty = true;
        }
    }
    pub fn remove(&mut self, index: usize) {
        if index < self.root.items().len() && editable(self.root.items()[index].name()) {
            self.root.items_mut().remove(index);
            self.dirty = true;
        }
    }
    pub fn warnings(&self) -> Vec<String> {
        let mut result = vec![];
        for name in [
            "sheet",
            "bus",
            "bus_entry",
            "image",
            "text_box",
            "rule_area",
            "table",
            "polyline",
        ] {
            let count = self.root.children(name).count();
            if count > 0 {
                result.push(format!(
                    "{count} {name} object(s) preserved; some advanced edits are not supported."
                ));
            }
        }
        if self
            .root
            .child("version")
            .is_some_and(|n| n.number(1) > 20250114.0)
        {
            result.push("Newer KiCad file version: unknown fields are preserved; full compatibility is not yet certified.".into());
        }
        result
    }
}
impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
fn update_reference(n: &mut Node, value: &str) {
    if n.name() == "reference" && n.items().len() > 1 {
        n.items_mut()[1] = Node::string(value);
    }
    if let Node::List(xs) = n {
        for x in xs.iter_mut().skip(1) {
            update_reference(x, value);
        }
    }
}
pub fn property<'a>(n: &'a Node, key: &str) -> &'a str {
    n.children("property")
        .find(|p| p.value(1) == key)
        .map(|p| p.value(2))
        .unwrap_or("")
}
pub fn editable(kind: &str) -> bool {
    matches!(
        kind,
        "symbol"
            | "wire"
            | "junction"
            | "no_connect"
            | "label"
            | "global_label"
            | "hierarchical_label"
            | "text"
    )
}
pub fn atomic_write(path: &Path, data: &[u8], backup: bool) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(data)?;
    tmp.as_file().sync_all()?;
    if backup && path.exists() {
        let backup_path = path.with_file_name(format!(
            "{}.jenga-backup",
            path.file_name().unwrap().to_string_lossy()
        ));
        fs::copy(path, backup_path).context("Create backup before replacing schematic")?;
    }
    let mut retries = 0;
    loop {
        match tmp.persist(path) {
            Ok(_) => break,
            Err(e)
                if cfg!(windows)
                    && matches!(e.error.raw_os_error(), Some(5 | 32 | 33))
                    && retries < 5 =>
            {
                // Short Windows scanner/indexer sharing locks can occur immediately
                // after a project rename. Retry this atomic replacement only.
                tmp = e.file;
                retries += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(anyhow::anyhow!("Replace {}: {}", path.display(), e.error)),
        }
    }
    Ok(())
}
pub fn create_project(folder: &Path, name: &str) -> Result<Document> {
    if name.trim().is_empty()
        || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        || name == "."
        || name == ".."
    {
        bail!("Use a nonempty project name without Windows path characters.");
    }
    let dir = folder.join(name);
    if dir.exists() && (!dir.is_dir() || fs::read_dir(&dir)?.next().is_some()) {
        bail!(
            "{} already exists; choose another project name.",
            dir.display()
        );
    }
    fs::create_dir_all(&dir)?;
    let project = serde_json::json!({"meta":{"filename":format!("{name}.kicad_pro"),"version":1},"net_settings":{"classes":[]},"schematic":{}});
    atomic_write(
        &dir.join(format!("{name}.kicad_pro")),
        serde_json::to_string_pretty(&project)?.as_bytes(),
        false,
    )?;
    let mut doc = Document::new();
    doc.save(&dir.join(format!("{name}.kicad_sch")))?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markers_use_native_two_coordinate_positions_after_moving() {
        let mut d = Document::new();
        for kind in ["junction", "no_connect"] {
            d.add_point(kind, 10., 20.);
            let i = d.root.items().len() - 1;
            assert_eq!(d.root.items()[i].child("at").unwrap().items().len(), 3);
            d.translate(i, 2., 3.);
            assert_eq!(d.root.items()[i].child("at").unwrap().items().len(), 3);
            assert_eq!(position(&d.root.items()[i]), (12., 23., 0.));
        }
    }
    #[test]
    fn save_preserves_unknown_fields_and_makes_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.kicad_sch");
        let mut d = Document::new();
        d.root
            .items_mut()
            .push(parse("(future (payload \"Ω\"))").unwrap());
        d.save(&path).unwrap();
        let first = fs::read_to_string(&path).unwrap();
        d.add_wire((10., 10.), (20., 10.));
        d.save(&path).unwrap();
        assert_eq!(
            fs::read_to_string(path.with_file_name("test.kicad_sch.jenga-backup")).unwrap(),
            first
        );
        assert!(
            Document::open(&path)
                .unwrap()
                .root
                .child("future")
                .is_some()
        );
    }
    #[test]
    fn movement_preserves_symbol_property_offsets() {
        let mut d = Document::new();
        let def=parse("(symbol \"R\" (property \"Reference\" \"R\" (at 2 1 0)) (property \"Value\" \"R\" (at 0 0 0)))").unwrap();
        let i = d.add_symbol("Device:R", &def, 20., 20., 1);
        d.translate(i, 5., -2.);
        assert_eq!(position(&d.root.items()[i]), (25., 18., 0.));
        assert_eq!(
            position(d.root.items()[i].children("property").next().unwrap()),
            (27., 17., 0.)
        );
    }
}
