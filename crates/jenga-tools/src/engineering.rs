//! Evidence-backed review records. CAD checks never imply bench validation.
use crate::engine::{Engine, string};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
pub fn request(e: &Engine, p: &Value) -> Result<Value> {
    let folder = e
        .board
        .as_ref()
        .and_then(|b| b.path.as_ref())
        .or_else(|| e.doc.as_ref()?.path.as_ref())
        .and_then(|p| p.parent())
        .context("Open a saved project first")?;
    let path = folder.join(".jenga/engineering.json");
    let record = if let Some(v) = p.get("record") {
        ensure!(v.is_object(), "record must be an object");
        let previous = std::fs::read_to_string(&path).unwrap_or_default();
        if !previous.is_empty() {
            ensure!(
                p["expected_revision"].as_str() == Some(&crate::journal::revision_text(&previous)),
                "Engineering record changed; read engineering_review before updating"
            );
        }
        let text = serde_json::to_string_pretty(v)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        jenga_core::document::atomic_write(&path, text.as_bytes(), true)?;
        v.clone()
    } else {
        std::fs::read_to_string(&path)
            .ok()
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or(json!({}))
    };
    let mut findings = vec![];
    for key in [
        "function",
        "external_interfaces",
        "operating_conditions",
        "ground_return_review",
        "antenna_review",
        "mechanical_review",
        "bench_test_plan",
    ] {
        if record[key].as_str().is_none_or(|s| s.trim().is_empty()) {
            findings.push(json!({"kind":"unresolved_review","field":key}));
        }
    }
    let components = record["components"].as_array().cloned().unwrap_or_default();
    let rails = record["rails"].as_array().cloned().unwrap_or_default();
    if rails.is_empty() {
        findings.push(json!({"kind":"missing_power_budget"}));
    }
    for rail in &rails {
        let name = rail["name"].as_str().unwrap_or("?");
        let capacity = rail["capacity_ma"].as_f64();
        let mut load = 0.;
        let mut unknown = false;
        for c in components.iter().filter(|c| c["rail"] == rail["name"]) {
            match c["peak_ma"].as_f64().filter(|v| v.is_finite() && *v >= 0.) {
                Some(v) => load += v,
                None => unknown = true,
            }
            match (
                rail["min_v"].as_f64(),
                rail["max_v"].as_f64(),
                c["min_v"].as_f64(),
                c["max_v"].as_f64(),
            ) {
                (Some(lo), Some(hi), Some(a), Some(b)) if lo <= hi && a <= b => {
                    if lo < a || hi > b {
                        findings.push(json!({"kind":"voltage_mismatch","reference":c["reference"],"rail":name,"supply_range_v":[lo,hi],"component_range_v":[a,b]}));
                    }
                }
                _ => findings.push(
                    json!({"kind":"unknown_voltage_limits","reference":c["reference"],"rail":name}),
                ),
            }
        }
        let margin = rail["margin_fraction"].as_f64().unwrap_or(0.2);
        ensure!(
            (0.0..=2.0).contains(&margin),
            "margin_fraction must be 0 to 2"
        );
        match capacity.filter(|v| v.is_finite() && *v > 0.) {
            Some(c) if !unknown => {
                if load * (1. + margin) > c {
                    findings.push(json!({"kind":"power_budget_exceeded","rail":name,"peak_load_ma":load,"required_with_margin_ma":load*(1.+margin),"capacity_ma":c}));
                }
            }
            _ => findings.push(json!({"kind":"unknown_power_budget","rail":name})),
        }
    }
    for c in &components {
        for key in [
            "reference",
            "manufacturer",
            "part_number",
            "datasheet",
            "pinout_review",
            "footprint_review",
            "logic_review",
        ] {
            if c[key].as_str().is_none_or(|s| s.trim().is_empty()) {
                findings.push(json!({"kind":"missing_component_evidence","reference":c["reference"],"field":key}));
            }
        }
        if !rails.iter().any(|r| r["name"] == c["rail"]) {
            findings.push(
                json!({"kind":"unknown_supply_rail","reference":c["reference"],"rail":c["rail"]}),
            );
        }
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(
        json!({"record":record,"path":path,"revision":crate::journal::revision_text(&text),"findings":findings,"review_complete":!components.is_empty()&&findings.is_empty(),"hardware_validated":false,"note":"User/agent supplied evidence; calculations do not authenticate datasheets or prove hardware operation. Run native ERC/DRC separately."}),
    )
}
/// Bounded HTTPS data download, never executable code or archives.
pub fn download(
    url: &str,
    name: &str,
    max: u64,
) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    ensure!(
        url.starts_with("https://") && !url.contains(['\r', '\n']),
        "Use an HTTPS data URL"
    );
    crate::projects::valid_name(name)?;
    let temp = tempfile::tempdir()?;
    let path = temp.path().join(name);
    let mut command = std::process::Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    command
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
            "60",
            "--max-filesize",
            &max.to_string(),
            "--output",
        ])
        .arg(&path)
        .arg(url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output()?;
    ensure!(
        output.status.success(),
        "Asset download failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        std::fs::metadata(&path)?.len() <= max,
        "Asset exceeds size limit"
    );
    Ok((temp, path))
}
pub fn import_asset(e: &mut Engine, p: &Value) -> Result<Value> {
    let url = p["url"].as_str();
    let downloaded = if let Some(url) = url {
        Some(download(url, string(p, "filename")?, 256 * 1024 * 1024)?)
    } else {
        None
    };
    let source = downloaded
        .as_ref()
        .map(|(_, p)| p.clone())
        .unwrap_or_else(|| e.resolve(p["path"].as_str().unwrap_or("")));
    ensure!(
        source.is_file(),
        "Provide an existing data file path or HTTPS url and filename"
    );
    let ext = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mut q = p.clone();
    q["path"] = json!(source);
    q.as_object_mut().unwrap().remove("url");
    let mut result = match ext.as_str() {
        "kicad_sym" | "kicad_mod" => {
            q["source"] = json!(std::fs::read_to_string(&source)?);
            if let Some(url) = url {
                q["url"] = json!(url);
            }
            e.request("import_library", &q)?
        }
        "step" | "stp" | "wrl" | "glb" | "stl" | "obj" => {
            crate::footprint_tools::import_model(e, &q)?
        }
        _ => anyhow::bail!(
            "Unsupported asset extension {ext}; provide unpacked KiCad symbols/footprints or a supported 3D data file"
        ),
    };
    result["source_url"] = p["url"].clone();
    result["attribution"] = p["attribution"].clone();
    result["license"] = p["license"].clone();
    if let Some(dest) = result["path"].as_str() {
        let provenance = std::path::PathBuf::from(format!("{dest}.source.json"));
        jenga_core::document::atomic_write(&provenance,serde_json::to_vec_pretty(&json!({"url":p["url"],"source_path":p["path"],"license":p["license"],"attribution":p["attribution"],"imported_ms":crate::journal::now()}))?.as_slice(),false)?;
    }
    Ok(result)
}
