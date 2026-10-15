//! Library source discovery and symbol inheritance, independent of desktop UI.
use anyhow::{Context, Result, bail};
use jenga_core::sexpr::{Node, parse};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    pub path: PathBuf,
    pub kind: String,
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub file: PathBuf,
    pub source: String,
}
pub struct Libraries {
    pub sources: Vec<Source>,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
    files: HashMap<PathBuf, Node>,
}
impl Libraries {
    pub fn discover(extra: &[Source], project: Option<&Path>) -> Self {
        let mut sources = extra.to_vec();
        if let Some(dir) = project {
            for (file, kind) in [("sym-lib-table", "symbols"), ("fp-lib-table", "footprints")] {
                read_table(&dir.join(file), "Project", kind, &mut sources, Some(dir));
            }
        }
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let base = PathBuf::from(appdata).join("kicad");
            if let Ok(dirs) = fs::read_dir(&base) {
                let mut dirs: Vec<_> = dirs
                    .flatten()
                    .map(|d| d.path())
                    .filter(|p| p.is_dir())
                    .collect();
                dirs.sort_by_key(|p| version_key(p));
                dirs.reverse();
                for dir in dirs {
                    for (file, kind) in
                        [("sym-lib-table", "symbols"), ("fp-lib-table", "footprints")]
                    {
                        read_table(&dir.join(file), "KiCad custom", kind, &mut sources, project);
                    }
                }
            }
        }
        let program = std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Program Files"));
        if let Ok(dirs) = fs::read_dir(program.join("KiCad")) {
            let mut dirs: Vec<_> = dirs.flatten().map(|d| d.path()).collect();
            dirs.sort_by_key(|p| version_key(p));
            dirs.reverse();
            for dir in dirs {
                let base = dir.join("share/kicad");
                for (sub, kind) in [
                    ("symbols", "symbols"),
                    ("footprints", "footprints"),
                    ("3dmodels", "3dmodels"),
                ] {
                    if base.join(sub).is_dir() {
                        sources.push(Source {
                            name: format!(
                                "KiCad {}",
                                dir.file_name().unwrap_or_default().to_string_lossy()
                            ),
                            path: base.join(sub),
                            kind: kind.into(),
                        });
                    }
                }
            }
        }
        let exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_owned));
        let development_root = exe
            .as_ref()
            .filter(|p| {
                p.ancestors()
                    .any(|a| a.file_name().is_some_and(|n| n == "target"))
            })
            .map(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/libraries"));
        for base in [exe.map(|p| p.join("libraries")), development_root]
            .into_iter()
            .flatten()
        {
            for kind in ["symbols", "footprints", "3dmodels"] {
                let path = base.join(kind);
                if path.is_dir() {
                    sources.push(Source {
                        name: "Jenga bundled".into(),
                        path,
                        kind: kind.into(),
                    });
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        sources.retain(|s| seen.insert((s.path.clone(), s.kind.clone())));
        Self {
            sources,
            entries: vec![],
            warnings: vec![],
            files: HashMap::new(),
        }
    }
    pub fn index(&mut self) -> Result<()> {
        self.entries.clear();
        self.files.clear();
        self.warnings.clear();
        let mut entries = BTreeMap::new();
        let mut indexed = std::collections::HashSet::new();
        for source in &self.sources {
            if source.kind != "symbols" {
                continue;
            }
            let paths = if source.path.is_file() {
                vec![source.path.clone()]
            } else {
                fs::read_dir(&source.path)
                    .map(|xs| {
                        xs.flatten()
                            .map(|e| e.path())
                            .filter(|p| p.extension().is_some_and(|x| x == "kicad_sym"))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            if paths.is_empty() {
                self.warnings
                    .push(format!("No supported symbols at {}", source.path.display()));
            }
            for path in paths {
                let lib = if source.path.is_file() {
                    source
                        .name
                        .split(" / ")
                        .last()
                        .unwrap_or("Custom")
                        .to_owned()
                } else {
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                };
                if !indexed.insert((path.clone(), lib.clone())) {
                    continue;
                }
                let result = fs::read_to_string(&path)
                    .context("read library")
                    .and_then(|s| parse(&s));
                match result {
                    Ok(root) => {
                        for symbol in root.children("symbol") {
                            let id = format!("{lib}:{}", symbol.value(1));
                            entries.entry(id.clone()).or_insert(Entry {
                                id,
                                file: path.clone(),
                                source: source.name.clone(),
                            });
                        }
                        // Keep only small, frequently used libraries cached; others load on demand.
                        if matches!(lib.as_str(), "Device" | "power" | "Connector_Generic") {
                            self.files.insert(path, root);
                        }
                    }
                    Err(e) => self.warnings.push(format!("{}: {e}", path.display())),
                }
            }
        }
        self.entries = entries.into_values().collect();
        Ok(())
    }
    pub fn symbol(&mut self, id: &str) -> Result<Node> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .with_context(|| format!("Symbol {id} not found. Check library sources."))?;
        if !self.files.contains_key(&entry.file) {
            self.files.insert(
                entry.file.clone(),
                parse(&fs::read_to_string(&entry.file)?)?,
            );
        }
        let root = &self.files[&entry.file];
        let name = id.split_once(':').map(|(_, n)| n).unwrap_or(id);
        resolve(root, name, 0)
    }
}
fn resolve(root: &Node, name: &str, depth: usize) -> Result<Node> {
    if depth > 32 {
        bail!("Symbol inheritance cycle");
    }
    let mut symbol = root
        .children("symbol")
        .find(|s| s.value(1) == name)
        .cloned()
        .with_context(|| format!("Missing inherited symbol {name}"))?;
    if let Some(parent) = symbol.child("extends") {
        let parent = resolve(root, parent.value(1), depth + 1)?;
        let own_props: Vec<_> = symbol
            .children("property")
            .map(|p| p.value(1).to_owned())
            .collect();
        for child in parent.items().iter().skip(2) {
            if child.name() == "extends" {
                continue;
            }
            if child.name() == "property" {
                if !own_props.iter().any(|p| p == child.value(1)) {
                    symbol.items_mut().push(child.clone());
                }
            } else if child.name() == "symbol" || symbol.child(child.name()).is_none() {
                symbol.items_mut().push(child.clone());
            }
        }
        symbol.items_mut().retain(|n| n.name() != "extends");
    }
    Ok(symbol)
}
fn read_table(path: &Path, label: &str, kind: &str, out: &mut Vec<Source>, project: Option<&Path>) {
    read_table_depth(path, label, kind, out, project, 0);
}
fn read_table_depth(
    path: &Path,
    label: &str,
    kind: &str,
    out: &mut Vec<Source>,
    project: Option<&Path>,
    depth: usize,
) {
    if depth > 8 {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let Ok(root) = parse(&text) else {
        return;
    };
    for lib in root.children("lib") {
        if lib.child("disabled").is_some() {
            continue;
        }
        let name = lib.child("name").map(|n| n.value(1)).unwrap_or("Custom");
        let uri = lib.child("uri").map(|n| n.value(1)).unwrap_or("");
        let mut expanded = uri.to_owned();
        if let Some(dir) = project {
            expanded = expanded.replace("${KIPRJMOD}", &dir.to_string_lossy());
        }
        for (key, value) in std::env::vars() {
            expanded = expanded.replace(&format!("${{{key}}}"), &value);
        }
        // KiCad's path variables can live in its common settings rather than process environment.
        if let Some(parent) = path.parent()
            && let Ok(text) = fs::read_to_string(parent.join("kicad_common.json"))
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
            && let Some(vars) = v.pointer("/environment/vars").and_then(|v| v.as_object())
        {
            for (key, value) in vars {
                if let Some(value) = value.as_str() {
                    expanded = expanded.replace(&format!("${{{key}}}"), value);
                }
            }
        }
        // Default install variables for this KiCad version.
        let version = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        for (suffix, sub) in [
            ("SYMBOL_DIR", "symbols"),
            ("FOOTPRINT_DIR", "footprints"),
            ("3DMODEL_DIR", "3dmodels"),
        ] {
            let major = version.split('.').next().unwrap_or("10");
            expanded = expanded.replace(
                &format!("${{KICAD{major}_{suffix}}}"),
                &format!("C:/Program Files/KiCad/{version}/share/kicad/{sub}"),
            );
            // The default table is in the installation's template folder.
            if let Some(install) = path.parent().and_then(|p| p.parent()) {
                let candidate = install.join(sub);
                if candidate.is_dir() {
                    expanded = expanded.replace(
                        &format!("${{KICAD10_{suffix}}}"),
                        &candidate.to_string_lossy(),
                    );
                }
            }
        }
        let p = PathBuf::from(expanded);
        if p.exists() {
            if lib.child("type").is_some_and(|t| t.value(1) == "Table") {
                read_table_depth(&p, label, kind, out, project, depth + 1);
            } else {
                out.push(Source {
                    name: format!("{label} / {name}"),
                    path: p,
                    kind: kind.into(),
                });
            }
        }
    }
}
fn version_key(path: &Path) -> Vec<u32> {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .split('.')
        .map(|s| s.parse().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inheritance_keeps_parent_geometry_and_overrides_properties() {
        let n=parse("(kicad_symbol_lib (symbol \"Base\" (property \"Value\" \"Base\") (symbol \"Base_1_1\" (rectangle (start 0 0) (end 1 1)))) (symbol \"Child\" (extends \"Base\") (property \"Value\" \"Child\")))").unwrap();
        let child = resolve(&n, "Child", 0).unwrap();
        assert!(child.child("extends").is_none());
        assert_eq!(child.children("symbol").count(), 1);
        assert_eq!(child.children("property").next().unwrap().value(2), "Child");
    }
}
