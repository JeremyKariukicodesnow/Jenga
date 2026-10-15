//! Bounded native KiCad/OpenCascade bridge. The Windows release bundles this
//! backend; installed KiCad is also discovered. Native files remain portable.
use anyhow::{Context, Result, ensure};
use jenga_core::pcb::Board;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn executable() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("JENGA_KICAD_CLI") {
        let p = PathBuf::from(path);
        ensure!(
            p.is_file(),
            "JENGA_KICAD_CLI does not exist: {}",
            p.display()
        );
        return Ok(p);
    }
    let mut candidates = vec![];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("backend/kicad/bin/kicad-cli.exe"));
            candidates.push(dir.join("kicad-cli.exe"));
        }
    }
    if std::env::current_exe().is_ok_and(|p| {
        p.ancestors()
            .any(|a| a.file_name().is_some_and(|n| n == "target"))
    }) {
        for name in [
            "tools/kicad/bin/kicad-cli.exe",
            "resources/backend/kicad/bin/kicad-cli.exe",
        ] {
            candidates.push(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join(name),
            );
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for folder in std::env::split_paths(&path) {
            candidates.push(folder.join(if cfg!(windows) {
                "kicad-cli.exe"
            } else {
                "kicad-cli"
            }));
        }
    }
    #[cfg(windows)]
    {
        if let Some(base) = std::env::var_os("ProgramFiles") {
            let root = PathBuf::from(base).join("KiCad");
            let mut dirs: Vec<_> = std::fs::read_dir(root)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            dirs.sort();
            dirs.reverse();
            for d in dirs {
                candidates.push(d.join("bin/kicad-cli.exe"));
            }
        }
    }
    candidates.into_iter().find(|p|p.is_file()).context("Native PCB/3D backend unavailable. Use the complete Jenga release or set JENGA_KICAD_CLI to an installed KiCad 10 CLI")
}
pub fn run(args: &[String], timeout: Duration) -> Result<Value> {
    let exe = executable()?;
    let temp = tempfile::tempdir()?;
    let log = temp.path().join("backend.log");
    let stdout = std::fs::File::create(&log)?;
    let stderr = stdout.try_clone()?;
    let mut command = Command::new(&exe);
    command
        .args(args)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("Start PCB backend {}", exe.display()))?;
    let started = Instant::now();
    let status = loop {
        if let Err(e) = crate::operations::checkpoint() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "PCB backend timed out after {} seconds; no document edits committed",
                timeout.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let log: String = text.chars().take(16000).collect();
    ensure!(
        status.success(),
        "PCB backend exited {:?}: {}",
        status.code(),
        log
    );
    Ok(json!({"backend":exe,"log":log,"elapsed_ms":started.elapsed().as_millis()}))
}
pub fn model_roots() -> Vec<PathBuf> {
    let mut roots = vec![];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            roots.push(p.join("libraries/3dmodels"));
        }
        if exe
            .ancestors()
            .any(|a| a.file_name().is_some_and(|n| n == "target"))
        {
            roots.push(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../resources/libraries/3dmodels"),
            );
        }
    }
    if let Ok(exe) = executable() {
        if let Some(base) = exe.parent().and_then(|p| p.parent()) {
            roots.push(base.join("share/kicad/3dmodels"));
        }
    }
    for version in ["10", "9", "8", "7", "6"] {
        if let Some(path) = std::env::var_os(format!("KICAD{version}_3DMODEL_DIR")) {
            roots.push(path.into());
        }
    }
    #[cfg(windows)]
    if let Some(base) = std::env::var_os("ProgramFiles") {
        if let Ok(dirs) = std::fs::read_dir(PathBuf::from(base).join("KiCad")) {
            for d in dirs.flatten() {
                roots.push(d.path().join("share/kicad/3dmodels"));
            }
        }
    }
    roots.retain(|p| p.is_dir());
    roots
}
pub fn resolve_model(path: &str, folder: &Path) -> Option<PathBuf> {
    let mut value = path.replace("${KIPRJMOD}", &folder.to_string_lossy());
    for (key, var) in std::env::vars() {
        if key.contains("3DMODEL") || key == "KIPRJMOD" {
            value = value.replace(&format!("${{{key}}}"), &var);
        }
    }
    let p = PathBuf::from(&value);
    if p.is_file() {
        return Some(jenga_core::document::normalized_path(&p));
    }
    if !p.is_absolute() && folder.join(&p).is_file() {
        return Some(jenga_core::document::normalized_path(&folder.join(p)));
    }
    // Keep the original variable token: an obsolete KiCad environment path
    // must not prevent resolution through the complete bundled model library.
    if let Some((variable, rest)) = path
        .split_once("}")
        .filter(|(variable, _)| variable.contains("3DMODEL"))
    {
        let _ = variable;
        let rest = rest.trim_start_matches(['/', '\\']);
        for root in model_roots() {
            let p = root.join(rest);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    for root in model_roots() {
        let p = root.join(&value);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}
pub fn prepared(board: &Board, dir: &Path) -> Result<PathBuf> {
    let mut root = board.root.clone();
    let folder = board
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .unwrap_or(Path::new("."));
    for f in root
        .items_mut()
        .iter_mut()
        .filter(|n| n.name() == "footprint")
    {
        for m in f.items_mut().iter_mut().filter(|n| n.name() == "model") {
            if let Some(path) = resolve_model(m.value(1), folder) {
                m.items_mut()[1] = jenga_core::sexpr::Node::string(path.to_string_lossy());
            }
        }
    }
    let path = dir.join("board.kicad_pcb");
    // Resolve the same project/bundled libraries as the editor, using absolute
    // paths because the native backend operates in an isolated temporary folder.
    let libs = jenga_libraries::Libraries::discover(&[], Some(folder));
    for (table, kind) in [("fp-lib-table", "footprints"), ("sym-lib-table", "symbols")] {
        let mut entries = std::collections::BTreeMap::new();
        for source in libs.sources.iter().filter(|s| s.kind == kind) {
            if source
                .path
                .extension()
                .is_some_and(|e| e == "pretty" || e == "kicad_sym")
            {
                let name = source.name.split(" / ").last().unwrap_or(&source.name);
                entries
                    .entry(name.to_owned())
                    .or_insert(source.path.clone());
            } else if let Ok(children) = std::fs::read_dir(&source.path) {
                for child in children.flatten() {
                    let p = child.path();
                    if p.extension()
                        .is_some_and(|e| e == "pretty" || e == "kicad_sym")
                    {
                        if let Some(name) = p.file_stem().and_then(|n| n.to_str()) {
                            entries.entry(name.to_owned()).or_insert(p);
                        }
                    }
                }
            }
        }
        let mut n = jenga_core::pcb::node(
            if kind == "footprints" {
                "fp_lib_table"
            } else {
                "sym_lib_table"
            },
            vec![jenga_core::pcb::node(
                "version",
                vec![jenga_core::sexpr::Node::atom(7)],
            )],
        );
        for (name, uri) in entries {
            n.items_mut().push(jenga_core::pcb::node(
                "lib",
                vec![
                    jenga_core::pcb::node("name", vec![jenga_core::sexpr::Node::string(name)]),
                    jenga_core::pcb::node("type", vec![jenga_core::sexpr::Node::string("KiCad")]),
                    jenga_core::pcb::node(
                        "uri",
                        vec![jenga_core::sexpr::Node::string(uri.to_string_lossy())],
                    ),
                    jenga_core::pcb::node("options", vec![jenga_core::sexpr::Node::string("")]),
                ],
            ));
        }
        std::fs::write(dir.join(table), n.document_text())?;
    }
    jenga_core::document::atomic_write(&path, root.document_text().as_bytes(), false)?;
    let mut pro=board.path.as_ref().and_then(|p|std::fs::read(p.with_extension("kicad_pro")).ok()).and_then(|s|serde_json::from_slice::<Value>(&s).ok()).unwrap_or(json!({"meta":{"filename":"board.kicad_pro","version":1},"board":{"design_settings":{"rules":{}}}}));
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
    rules["min_clearance"] = json!(board.rules.clearance);
    rules["min_track_width"] = json!(board.rules.track_width);
    rules["min_copper_edge_clearance"] = json!(board.rules.edge_clearance);
    jenga_core::document::atomic_write(
        &path.with_extension("kicad_pro"),
        &serde_json::to_vec_pretty(&pro)?,
        false,
    )?;
    if let Some(source) = board
        .path
        .as_ref()
        .map(|p| p.with_extension("kicad_dru"))
        .filter(|p| p.is_file())
    {
        std::fs::copy(source, path.with_extension("kicad_dru"))?;
    }
    Ok(path)
}
pub fn mesh(board: &Board, path: &Path) -> Result<Value> {
    let temp = tempfile::tempdir()?;
    let input = prepared(board, temp.path())?;
    let args = vec![
        "pcb".into(),
        "export".into(),
        "glb".into(),
        "--force".into(),
        "--include-tracks".into(),
        "--include-pads".into(),
        "--include-zones".into(),
        "--include-soldermask".into(),
        "--include-silkscreen".into(),
        "--cut-vias-in-body".into(),
        "--subst-models".into(),
        "--output".into(),
        path.to_string_lossy().into(),
        input.to_string_lossy().into(),
    ];
    let mut result = run(&args, Duration::from_secs(120))?;
    ensure!(path.is_file(), "3D backend produced no GLB assembly");
    result["path"] = json!(path);
    result["models"] = model_findings(board);
    Ok(result)
}
pub fn model_findings(board: &Board) -> Value {
    let folder = board
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .unwrap_or(Path::new("."));
    let mut results = vec![];
    for f in board.root.children("footprint") {
        let models: Vec<_> = f.children("model").collect();
        if models.is_empty() {
            let pads: Vec<_> = f.children("pad").collect();
            let mechanical = !pads.is_empty() && pads.iter().all(|p| p.value(2) == "np_thru_hole");
            results.push(
                json!({"reference":jenga_core::pcb::field(f,"Reference"),"status":if mechanical {"not_required"} else {"unassigned"},"reason":if mechanical {"Nonplated mechanical holes do not need a component model"} else {"No component model assigned"}}),
            );
        }
        for m in models {
            let resolved = resolve_model(m.value(1), folder);
            results.push(json!({"reference":jenga_core::pcb::field(f,"Reference"),"path":m.value(1),"resolved_path":resolved,"status":if resolved.is_some(){"available"}else{"missing"}}));
        }
    }
    json!(results)
}
pub fn mesh_revision(board: &Board) -> String {
    let findings = model_findings(board);
    let stamps: Vec<_> = findings
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| {
            let path = m["resolved_path"].as_str().map(PathBuf::from);
            let stamp = path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| {
                    (
                        m.len(),
                        m.modified()
                            .ok()
                            .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|t| t.as_nanos()),
                    )
                });
            json!({"model":m,"stamp":stamp})
        })
        .collect();
    jenga_core::pcb::revision_text(
        &json!({"board":board.revision(),"file":board.path,"models":stamps}).to_string(),
    )
}
pub fn footprint_mesh(
    def: &jenga_core::sexpr::Node,
    folder: &Path,
    path: &Path,
    pads: bool,
) -> Result<Value> {
    let mut b = Board::new(20., 20.)?;
    b.path = Some(folder.join("preview.kicad_pcb"));
    b.add_footprint("Preview:Footprint", def, "REF", "", (0., 0.), 0.)?;
    let temp = tempfile::tempdir()?;
    let native = prepared(&b, temp.path())?;
    let mut args = vec![
        "pcb".into(),
        "export".into(),
        "glb".into(),
        "--no-board-body".into(),
        "--force".into(),
        "--subst-models".into(),
        "--output".into(),
        path.to_string_lossy().into(),
        native.to_string_lossy().into(),
    ];
    if pads {
        args.insert(3, "--include-pads".into());
    }
    let mut result = run(&args, Duration::from_secs(120))?;
    result["models"] = model_findings(&b);
    Ok(result)
}
pub fn drc(board: &Board, refill: bool) -> Result<(Value, Option<jenga_core::sexpr::Node>)> {
    let temp = tempfile::tempdir()?;
    let input = prepared(board, temp.path())?;
    let output = temp.path().join("drc.json");
    let mut args = vec![
        "pcb".into(),
        "drc".into(),
        "--format".into(),
        "json".into(),
        "--output".into(),
        output.to_string_lossy().into(),
    ];
    if refill {
        args.extend(["--refill-zones".into(), "--save-board".into()]);
    }
    args.push(input.to_string_lossy().into());
    let backend = run(&args, Duration::from_secs(90))?;
    let mut report: Value = serde_json::from_slice(&std::fs::read(output)?)?;
    report["backend"] = backend;
    let root = if refill {
        let mut root = jenga_core::sexpr::parse(&std::fs::read_to_string(input)?)?;
        for f in root
            .items_mut()
            .iter_mut()
            .filter(|n| n.name() == "footprint")
        {
            if let Some(old) = board
                .root
                .children("footprint")
                .find(|old| jenga_core::pcb::id(old) == jenga_core::pcb::id(f))
            {
                let models: Vec<_> = old.children("model").cloned().collect();
                f.items_mut().retain(|n| n.name() != "model");
                f.items_mut().extend(models);
            }
        }
        let mut converted = board.snapshot();
        converted.root = root;
        preserve_pad_nets(board, &mut converted)?;
        Some(converted.root)
    } else {
        None
    };
    Ok((report, root))
}
pub fn export(board: &Board, path: &Path, format: &str) -> Result<Value> {
    let temp = tempfile::tempdir()?;
    let input = prepared(board, temp.path())?;
    let mut args = vec!["pcb".into(), "export".into(), format.into()];
    match format {
        "gerbers" | "drill" => {
            std::fs::create_dir_all(path)?;
        }
        "step" | "glb" | "stl" | "brep" | "ply" => args.extend([
            "--force".into(),
            "--subst-models".into(),
            "--include-tracks".into(),
            "--include-pads".into(),
            "--include-zones".into(),
        ]),
        "pos" => args.extend([
            "--format".into(),
            "csv".into(),
            "--units".into(),
            "mm".into(),
        ]),
        "svg" | "pdf" => {
            args.extend(["--layers".into(), "F.Cu,B.Cu,F.SilkS,Edge.Cuts".into()]);
        }
        "vrml" | "dxf" | "ipc2581" | "ipcd356" | "gencad" => {}
        _ => anyhow::bail!("Unsupported PCB export {format}"),
    };
    args.extend([
        "--output".into(),
        path.to_string_lossy().into(),
        input.to_string_lossy().into(),
    ]);
    let mut result = run(&args, Duration::from_secs(120))?;
    result["path"] = json!(path);
    result["format"] = json!(format);
    Ok(result)
}
/// Native zone filling may normalize a format, but must preserve electrical intent.
pub fn preserve_pad_nets(before: &Board, after: &mut Board) -> Result<()> {
    let prior = before.pads();
    let now = after.pads();
    ensure!(
        prior.len() == now.len(),
        "Native conversion changed the pad count; no board applied"
    );
    for old in prior {
        let new = now.iter().find(|p| p.uuid == old.uuid).with_context(|| {
            format!(
                "Native conversion lost {}.{} ({})",
                old.reference, old.number, old.uuid
            )
        })?;
        ensure!(
            new.number == old.number
                && new.reference == old.reference
                && (new.at.0 - old.at.0).abs() < 1e-5
                && (new.at.1 - old.at.1).abs() < 1e-5,
            "Native conversion moved or renumbered {}.{}; no board applied",
            old.reference,
            old.number
        );
        ensure!(
            new.net_name.is_empty() || new.net_name == old.net_name,
            "Native conversion changed {}.{} net {} to {}; no board applied",
            old.reference,
            old.number,
            old.net_name,
            new.net_name
        );
        let code = after.ensure_net(&old.net_name);
        let node = after.net_node(code);
        let pad = after.root.items_mut()[new.object]
            .items_mut()
            .iter_mut()
            .find(|n| n.name() == "pad" && jenga_core::pcb::id(n) == old.uuid)
            .context("Converted pad UUID disappeared")?;
        pad.set_child(node);
    }
    Ok(())
}
