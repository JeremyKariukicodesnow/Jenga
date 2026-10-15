//! Deterministic, portable native VRML component bodies authored by humans or AI.
use crate::engine::{Engine, string};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
fn triple(v: &Value, default: [f64; 3]) -> Result<[f64; 3]> {
    if v.is_null() {
        return Ok(default);
    }
    let a = v
        .as_array()
        .context("Model coordinates need three numbers")?;
    ensure!(a.len() == 3, "Model coordinates need three numbers");
    let r = [
        a[0].as_f64().context("Invalid coordinate")?,
        a[1].as_f64().context("Invalid coordinate")?,
        a[2].as_f64().context("Invalid coordinate")?,
    ];
    ensure!(
        r.iter().all(|v| v.is_finite() && v.abs() <= 10000.),
        "Model coordinates must be finite and within 10000 mm"
    );
    Ok(r)
}
pub fn create(e: &Engine, p: &Value) -> Result<Value> {
    let name = string(p, "name")?;
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        "Model name uses letters, digits, underscores or hyphens"
    );
    let shapes = p["shapes"]
        .as_array()
        .context("model_create needs shapes: box, cylinder or sphere")?;
    ensure!(
        !shapes.is_empty() && shapes.len() <= 256,
        "Use 1–256 model shapes"
    );
    let mut text = String::from(
        "#VRML V2.0 utf8\n# Jenga component geometry; input millimetres, native VRML units 0.1 inch\n",
    );
    for (i, s) in shapes.iter().enumerate() {
        let at = triple(&s["at_mm"], [0.; 3])?;
        let color = triple(&s["color"], [0.15, 0.15, 0.15])?;
        ensure!(
            color.iter().all(|c| *c >= 0. && *c <= 1.),
            "Shape {i}: color channels must be 0–1"
        );
        let size = triple(&s["size_mm"], [1.; 3])?;
        ensure!(
            size.iter().all(|c| *c > 0.),
            "Shape {i}: dimensions must be positive"
        );
        let radius = s["radius_mm"].as_f64().unwrap_or(0.5);
        let height = s["height_mm"].as_f64().unwrap_or(1.);
        ensure!(
            radius.is_finite()
                && height.is_finite()
                && radius > 0.
                && height > 0.
                && radius <= 10000.
                && height <= 10000.,
            "Shape {i}: invalid radius or height"
        );
        let (geometry, orientation) = match s["kind"].as_str().unwrap_or("box") {
            "box" => (
                format!(
                    "Box {{ size {} {} {} }}",
                    size[0] / 2.54,
                    size[1] / 2.54,
                    size[2] / 2.54
                ),
                "0 0 1 0",
            ),
            "cylinder" => (
                format!(
                    "Cylinder {{ radius {} height {} }}",
                    radius / 2.54,
                    height / 2.54
                ),
                match s["axis"].as_str().unwrap_or("z") {
                    "x" => "0 0 1 -1.5707963267948966",
                    "y" => "0 0 1 0",
                    "z" => "1 0 0 1.5707963267948966",
                    _ => bail!("Shape {i}: axis must be x, y or z"),
                },
            ),
            "sphere" => (format!("Sphere {{ radius {} }}", radius / 2.54), "0 0 1 0"),
            other => bail!("Shape {i}: unsupported primitive {other}"),
        };
        text.push_str(&format!("Transform {{ translation {} {} {} rotation {orientation} children [ Shape {{ appearance Appearance {{ material Material {{ diffuseColor {} {} {} }} }} geometry {geometry} }} ] }}\n",at[0]/2.54,at[1]/2.54,at[2]/2.54,color[0],color[1],color[2]));
    }
    let folder = if p["scope"] == "personal" {
        crate::symbol_editor::personal_folder()?
    } else {
        p["project_folder"]
            .as_str()
            .map(|v| e.resolve(v))
            .unwrap_or_else(|| crate::footprint_tools::folder(e))
    };
    let dest = folder.join("models").join(format!("{name}.wrl"));
    ensure!(
        !dest.exists(),
        "Model destination exists; choose a new name: {}",
        dest.display()
    );
    std::fs::create_dir_all(dest.parent().unwrap())?;
    jenga_core::document::atomic_write(&dest, text.as_bytes(), false)?;
    Ok(
        json!({"path":dest,"reference":if p["scope"]=="personal" {dest.to_string_lossy().into_owned()} else {format!("${{KIPRJMOD}}/models/{name}.wrl")},"format":"vrml","units":"mm","shape_count":shapes.len(),"geometry_kind":"representative_component","hardware_dimensions_verified":false}),
    )
}
