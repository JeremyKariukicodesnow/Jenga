//! Camera-specific image and multi-view PDF exports from the native assembly.

use crate::engine::{Engine, string};

use anyhow::{Context, Result, bail, ensure};

use image::{ImageEncoder, Rgb, RgbImage};

use jenga_core::mesh::{Mesh, cross, sub, unit};

use serde_json::{Value, json};

use std::path::Path;

pub fn camera(view: &str) -> Result<(f64, f64)> {
    use std::f64::consts::{FRAC_PI_2, PI};

    Ok(match view {
        "isometric" => (0.6, 0.7),

        "top" => (0., FRAC_PI_2),

        "bottom" => (0., -FRAC_PI_2),

        "front" => (0., 0.),

        "back" => (PI, 0.),

        "left" => (FRAC_PI_2, 0.),

        "right" => (-FRAC_PI_2, 0.),

        _ => bail!(
            "Unknown 3D view {view}; use isometric, top, bottom, front, back, left, right or current"
        ),
    })
}

pub fn raster(mesh: &Mesh, w: u32, h: u32, yaw: f64, pitch: f64, zoom: f64) -> Result<RgbImage> {
    raster_lit(mesh, w, h, yaw, pitch, zoom, 1.2, 0.7)
}

pub fn raster_lit(
    mesh: &Mesh,
    w: u32,
    h: u32,
    yaw: f64,
    pitch: f64,
    zoom: f64,
    exposure: f64,
    ambient: f64,
) -> Result<RgbImage> {
    ensure!(
        exposure.is_finite()
            && (0.25..=4.).contains(&exposure)
            && ambient.is_finite()
            && (0.1..=1.).contains(&ambient),
        "brightness must be 0.25–4 and ambient 0.1–1"
    );

    ensure!(
        (320..=4096).contains(&w)
            && (240..=4096).contains(&h)
            && u64::from(w) * u64::from(h) <= 8_000_000,
        "Image dimensions must be 320–4096 × 240–4096 and at most 8 million pixels"
    );

    ensure!(
        yaw.is_finite() && pitch.is_finite() && zoom.is_finite() && zoom > 0. && zoom <= 30.,
        "Invalid 3D camera"
    );

    let (lo, hi) = mesh.bounds();

    let c = [
        (lo[0] + hi[0]) / 2.,
        (lo[1] + hi[1]) / 2.,
        (lo[2] + hi[2]) / 2.,
    ];

    let scale = f64::from(w.min(h))
        / ((hi[0] - lo[0])
            .hypot(hi[1] - lo[1])
            .max(hi[2] - lo[2])
            .max(1.)
            * 1.25)
        * zoom;

    let rotate = |v: [f64; 3]| {
        let x = v[0] - c[0];

        let y = v[1] - c[1];

        let z = v[2] - c[2];

        let a = x * yaw.cos() - y * yaw.sin();

        let b = x * yaw.sin() + y * yaw.cos();

        [
            a,
            b * pitch.sin() + z * pitch.cos(),
            b * pitch.cos() - z * pitch.sin(),
        ]
    };

    let edge = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
        (p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])
    };

    let mut out = RgbImage::from_pixel(w, h, Rgb([29, 34, 41]));

    let mut depths = vec![f64::INFINITY; (w * h) as usize];

    for t in &mesh.triangles {
        let v = t.vertices.map(rotate);

        let p = v.map(|v| {
            [
                f64::from(w) / 2. + v[0] * scale,
                f64::from(h) / 2. - v[1] * scale,
            ]
        });

        let area = edge(p[0], p[1], p[2]);

        if area.abs() < 1e-8 {
            continue;
        }

        let x0 = p
            .iter()
            .map(|p| p[0])
            .fold(f64::INFINITY, f64::min)
            .floor()
            .clamp(0., f64::from(w)) as u32;

        let y0 = p
            .iter()
            .map(|p| p[1])
            .fold(f64::INFINITY, f64::min)
            .floor()
            .clamp(0., f64::from(h)) as u32;

        let x1 = p
            .iter()
            .map(|p| p[0])
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .clamp(0., f64::from(w)) as u32;

        let y1 = p
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .clamp(0., f64::from(h)) as u32;

        let n = unit(cross(sub(v[1], v[0]), sub(v[2], v[0])));

        let color = Rgb(jenga_core::mesh::lit_color(t.color, n, exposure, ambient));

        for y in y0..y1 {
            for x in x0..x1 {
                let at = [f64::from(x) + 0.5, f64::from(y) + 0.5];

                let a = edge(p[1], p[2], at) / area;

                let b = edge(p[2], p[0], at) / area;

                let c = 1. - a - b;

                if a < -1e-8 || b < -1e-8 || c < -1e-8 {
                    continue;
                }

                let z = a * v[0][2] + b * v[1][2] + c * v[2][2];

                let i = (y * w + x) as usize;

                if z < depths[i] {
                    depths[i] = z;

                    out.put_pixel(x, y, color);
                }
            }
        }
    }

    Ok(out)
}

pub fn pdf(path: &Path, pages: &[(String, RgbImage)]) -> Result<()> {
    ensure!(!path.exists(), "PDF destination exists");

    ensure!(!pages.is_empty(), "No views to export");

    let kids = (0..pages.len())
        .map(|i| format!("{} 0 R", 4 + i * 3))
        .collect::<Vec<_>>()
        .join(" ");

    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Count {} /Kids [ {kids} ] >>", pages.len()).into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ];

    for (i, (name, img)) in pages.iter().enumerate() {
        let page = 4 + i * 3;

        let image = page + 1;

        let contents = page + 2;

        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 842 595] /Resources << /Font << /F1 3 0 R >> /XObject << /Im {image} 0 R >> >> /Contents {contents} 0 R >>").into_bytes());

        let mut png = vec![];

        image::codecs::png::PngEncoder::new(&mut png).write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
        )?;

        let mut data = vec![];

        let mut pos = 8;

        while pos + 12 <= png.len() {
            let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;

            ensure!(pos + 12 + len <= png.len(), "Invalid PNG data");

            if &png[pos + 4..pos + 8] == b"IDAT" {
                data.extend_from_slice(&png[pos + 8..pos + 8 + len]);
            }

            pos += 12 + len;
        }

        let mut object=format!("<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /DecodeParms << /Predictor 15 /Colors 3 /BitsPerComponent 8 /Columns {} >> /Length {} >>\nstream\n",img.width(),img.height(),img.width(),data.len()).into_bytes();

        object.extend(data);

        object.extend(b"\nendstream");

        objects.push(object);

        let scale = (782. / f64::from(img.width())).min(505. / f64::from(img.height()));

        let w = f64::from(img.width()) * scale;

        let h = f64::from(img.height()) * scale;

        let title: String = name
            .chars()
            .map(|c| if c.is_ascii() { c } else { '?' })
            .collect();

        let title = title
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");

        let content = format!(
            "BT /F1 16 Tf 30 564 Td ({title}) Tj ET\nq {w} 0 0 {h} {} {} cm /Im Do Q\n",
            (842. - w) / 2.,
            (540. - h) / 2.
        );

        objects.push(
            format!(
                "<< /Length {} >>\nstream\n{}endstream",
                content.len(),
                content
            )
            .into_bytes(),
        );
    }

    let mut bytes = b"%PDF-1.4\n".to_vec();

    let mut offsets = vec![0];

    for (i, o) in objects.iter().enumerate() {
        offsets.push(bytes.len());

        bytes.extend(format!("{} 0 obj\n", i + 1).as_bytes());

        bytes.extend(o);

        bytes.extend(b"\nendobj\n");
    }

    let xref = bytes.len();

    bytes.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());

    for o in offsets.iter().skip(1) {
        bytes.extend(format!("{o:010} 00000 n \n").as_bytes());
    }

    bytes.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len()
        )
        .as_bytes(),
    );

    jenga_core::document::atomic_write(path, &bytes, false)
}

pub fn export(e: &mut Engine, p: &Value) -> Result<Value> {
    let path = e.resolve(string(p, "path")?);

    ensure!(!path.exists(), "3D view export destination exists");

    let format = p["format"].as_str().unwrap_or("png");

    ensure!(
        matches!(format, "png" | "pdf"),
        "View export format must be png or pdf; use pcb_export for CAD models"
    );

    let assembly = crate::pcb_tools::request(e, "pcb_3d", &json!({}))?;

    let mesh = Mesh::read(Path::new(
        assembly["path"]
            .as_str()
            .context("Missing native assembly")?,
    ))?;

    let views: Vec<_> = p["views"]
        .as_array()
        .map(|v| v.iter().filter_map(Value::as_str).collect())
        .unwrap_or_else(|| vec![p["view"].as_str().unwrap_or("isometric")]);

    ensure!(!views.is_empty() && views.len() <= 7, "Choose 1–7 views");

    let mut pages = vec![];

    for view in &views {
        let (yaw, pitch) = if *view == "current" {
            (
                p["yaw"].as_f64().unwrap_or(0.6),
                p["pitch"].as_f64().unwrap_or(0.7),
            )
        } else {
            camera(view)?
        };

        let image = raster_lit(
            &mesh,
            p["width"].as_u64().unwrap_or(1600).min(4096) as u32,
            p["height"].as_u64().unwrap_or(1000).min(4096) as u32,
            yaw,
            pitch,
            p["zoom"].as_f64().unwrap_or(1.),
            p["brightness"].as_f64().unwrap_or(1.2),
            p["ambient"].as_f64().unwrap_or(0.7),
        )?;

        pages.push((format!("PCB 3D - {view}"), image));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut files = vec![];

    if format == "pdf" {
        pdf(&path, &pages)?;

        files.push(path.clone());
    } else if pages.len() == 1 {
        pages[0].1.save(&path)?;

        files.push(path.clone());
    } else {
        std::fs::create_dir(&path)?;

        for ((_, image), view) in pages.iter().zip(&views) {
            let out = path.join(format!("{view}.png"));

            ensure!(!out.exists(), "Duplicate view {view}");

            image.save(&out)?;

            files.push(out);
        }
    }

    Ok(
        json!({"path":path,"format":format,"views":views,"files":files,"page_count":pages.len(),"models":assembly["models"],"source_revision":assembly["revision"]}),
    )
}
