use crate::{
    document::{Document, atomic_write, property},
    scene::{Scene, Shape},
};
use anyhow::Result;
use std::{fmt::Write as _, path::Path};
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn color(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
pub fn svg(doc: &Document, path: &Path) -> Result<()> {
    let scene = Scene::from_document(doc);
    let mut out = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"297mm\" height=\"210mm\" viewBox=\"0 0 297 210\"><rect width=\"297\" height=\"210\" fill=\"#fffffb\"/>\n",
    );
    for draw in scene.draws {
        match draw.shape {
            Shape::Path {
                points,
                closed,
                fill,
                color: c,
                width,
            } => {
                let pts = points
                    .iter()
                    .map(|(x, y)| format!("{x},{y}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let tag = if closed { "polygon" } else { "polyline" };
                writeln!(
                    out,
                    "<{tag} points=\"{pts}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{width}\"/>",
                    fill.map(color).unwrap_or("none".into()),
                    color(c)
                )?;
            }
            Shape::Circle {
                center: (x, y),
                radius,
                fill,
                color: c,
                width,
            } => {
                writeln!(
                    out,
                    "<circle cx=\"{x}\" cy=\"{y}\" r=\"{radius}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{width}\"/>",
                    fill.map(color).unwrap_or("none".into()),
                    color(c)
                )?;
            }
            Shape::Text {
                at: (x, y),
                text,
                size,
                angle,
                color: c,
                left,
            } => {
                writeln!(
                    out,
                    "<text x=\"{x}\" y=\"{y}\" font-family=\"sans-serif\" font-size=\"{size}\" text-anchor=\"{}\" dominant-baseline=\"middle\" fill=\"{}\" transform=\"rotate({} {x} {y})\">{}</text>",
                    if left { "start" } else { "middle" },
                    color(c),
                    -angle,
                    xml(&text)
                )?;
            }
        }
    }
    out.push_str("</svg>\n");
    atomic_write(path, out.as_bytes(), false)
}
/// Minimal vector PDF writer: same scene as SVG, no KiCad executable dependency.
pub fn pdf(doc: &Document, path: &Path) -> Result<()> {
    let scene = Scene::from_document(doc);
    let k = 72. / 25.4;
    let h = 210. * k;
    let mut stream = String::new();
    for draw in scene.draws {
        match draw.shape {
            Shape::Path {
                points,
                closed,
                fill,
                color,
                width,
            } => {
                pdf_color(&mut stream, color, false);
                writeln!(stream, "{} w", width * k)?;
                if let Some(c) = fill {
                    pdf_color(&mut stream, c, true);
                }
                for (i, (x, y)) in points.iter().enumerate() {
                    writeln!(
                        stream,
                        "{} {} {}",
                        x * k,
                        h - y * k,
                        if i == 0 { "m" } else { "l" }
                    )?;
                }
                if closed {
                    stream.push_str("h\n");
                }
                stream.push_str(if fill.is_some() { "B\n" } else { "S\n" });
            }
            Shape::Circle {
                center: (x, y),
                radius: r,
                fill,
                color,
                width,
            } => {
                pdf_color(&mut stream, color, false);
                if let Some(c) = fill {
                    pdf_color(&mut stream, c, true);
                }
                writeln!(stream, "{} w", width * k)?;
                for i in 0..=48 {
                    let a = std::f64::consts::TAU * i as f64 / 48.;
                    writeln!(
                        stream,
                        "{} {} {}",
                        (x + r * a.cos()) * k,
                        h - (y + r * a.sin()) * k,
                        if i == 0 { "m" } else { "l" }
                    )?;
                }
                stream.push_str(if fill.is_some() { "B\n" } else { "S\n" });
            }
            Shape::Text {
                at: (mut x, y),
                text,
                size,
                angle,
                color,
                left,
            } => {
                pdf_color(&mut stream, color, true);
                if !left {
                    x -= text.chars().count() as f64 * size * 0.27;
                }
                let text = text
                    .chars()
                    .map(|c| if c.is_ascii() { c } else { '?' })
                    .collect::<String>()
                    .replace('\\', "\\\\")
                    .replace('(', "\\(")
                    .replace(')', "\\)")
                    .replace(['\n', '\r'], " ");
                let a = angle.to_radians();
                writeln!(
                    stream,
                    "BT /F1 {} Tf {} {} {} {} {} {} Tm ({}) Tj ET",
                    size * k,
                    a.cos(),
                    a.sin(),
                    -a.sin(),
                    a.cos(),
                    x * k,
                    h - y * k - size * k * 0.3,
                    text
                )?;
            }
        }
    }
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
            297. * k,
            h
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{}\nendobj\n", i + 1, obj).as_bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
    for offset in offsets.iter().skip(1) {
        out.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len()
        )
        .as_bytes(),
    );
    atomic_write(path, &out, false)
}
fn pdf_color(out: &mut String, c: [u8; 3], fill: bool) {
    let _ = writeln!(
        out,
        "{} {} {} {}",
        c[0] as f64 / 255.,
        c[1] as f64 / 255.,
        c[2] as f64 / 255.,
        if fill { "rg" } else { "RG" }
    );
}
pub fn bom(doc: &Document, path: &Path) -> Result<()> {
    let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut out = "Reference,Value,Footprint\n".to_owned();
    for n in doc.root.children("symbol") {
        if n.child("in_bom").is_some_and(|n| n.value(1) == "no") {
            continue;
        }
        out.push_str(&format!(
            "{},{},{}\n",
            quote(property(n, "Reference")),
            quote(property(n, "Value")),
            quote(property(n, "Footprint"))
        ));
    }
    atomic_write(path, out.as_bytes(), false)
}
