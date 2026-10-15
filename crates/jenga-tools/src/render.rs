//! CPU rendering for screenshots while the desktop app is closed.
use anyhow::{Result, ensure};
use image::{Rgb, RgbImage, imageops::FilterType};
use jenga_core::{
    document::Document,
    scene::{Scene, Shape},
};
use std::path::Path;
pub fn png(doc: &Document, path: &Path, width: u32) -> Result<()> {
    let scene = Scene::page_document(doc);
    let (w, h) = jenga_core::export::paper_size(doc);
    render(&scene, path, width, ((0., 0.), (w, h)))
}
pub fn preview(scene: &Scene, path: &Path, width: u32) -> Result<()> {
    let (a, b) = scene.content_bounds();
    render(
        scene,
        path,
        width,
        ((a.0 - 2., a.1 - 2.), (b.0 + 2., b.1 + 2.)),
    )
}
fn render(scene: &Scene, path: &Path, width: u32, bounds: ((f64, f64), (f64, f64))) -> Result<()> {
    ensure!(
        (320..=4096).contains(&width),
        "Screenshot width must be 320–4096"
    );

    ensure!(scene.missing.is_empty(), "Missing symbol definitions");
    let (a, b) = bounds;
    let (w, h) = (b.0 - a.0, b.1 - a.1);
    ensure!(
        w.is_finite() && h.is_finite() && w > 0. && h > 0.,
        "Invalid render bounds"
    );
    let maximum = (8_000_000_f64 * w / h).sqrt().floor().min(4096.) as u32;
    let width = width.min(maximum);
    let height = (width as f64 * h / w).floor().max(1.) as u32;
    ensure!(
        height >= 1 && (width as u64) * (height as u64) <= 8_000_000,
        "Screenshot bounds exceed the supported aspect ratio; use PDF/SVG or a smaller region"
    );
    let mut image = RgbImage::from_pixel(width * 2, height * 2, Rgb([255, 255, 251]));
    let scale = width as f64 * 2. / w;
    for draw in scene.draws.clone() {
        match draw.shape {
            Shape::Path {
                points,
                closed,
                fill,
                color,
                width,
            } => {
                let pts: Vec<_> = points
                    .iter()
                    .map(|p| ((p.0 - a.0) * scale, (p.1 - a.1) * scale))
                    .collect();
                if closed && let Some(c) = fill {
                    polygon(&mut image, &pts, c);
                }
                for pair in pts.windows(2) {
                    line(&mut image, pair[0], pair[1], (width * scale).max(1.), color);
                }
                if closed && pts.len() > 1 {
                    line(
                        &mut image,
                        *pts.last().unwrap(),
                        pts[0],
                        (width * scale).max(1.),
                        color,
                    );
                }
            }
            Shape::Circle {
                center,
                radius,
                fill,
                color,
                width,
            } => {
                let center = ((center.0 - a.0) * scale, (center.1 - a.1) * scale);
                let radius = radius * scale;
                if let Some(c) = fill {
                    disc(&mut image, center, radius, c);
                }
                let pts: Vec<_> = (0..=96)
                    .map(|i| {
                        let a = i as f64 * std::f64::consts::TAU / 96.;
                        (center.0 + radius * a.cos(), center.1 + radius * a.sin())
                    })
                    .collect();
                for p in pts.windows(2) {
                    line(&mut image, p[0], p[1], (width * scale).max(1.), color);
                }
            }
            Shape::Text { .. } => {} // Scene's NewStroke lettering is already emitted as vector paths.
        }
    }
    image::imageops::resize(&image, width, height, FilterType::Triangle).save(path)?;
    Ok(())
}
fn disc(image: &mut RgbImage, p: (f64, f64), r: f64, c: [u8; 3]) {
    let x0 = (p.0 - r).floor().max(0.) as i32;
    let x1 = (p.0 + r).ceil().min(image.width() as f64 - 1.) as i32;
    let y0 = (p.1 - r).floor().max(0.) as i32;
    let y1 = (p.1 + r).ceil().min(image.height() as f64 - 1.) as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if (x as f64 - p.0).powi(2) + (y as f64 - p.1).powi(2) <= r * r {
                image.put_pixel(x as u32, y as u32, Rgb(c));
            }
        }
    }
}
fn line(image: &mut RgbImage, a: (f64, f64), b: (f64, f64), width: f64, c: [u8; 3]) {
    // Clip before sampling: native schematics may contain geometry far off the sheet.
    if ![a.0, a.1, b.0, b.1, width].iter().all(|v| v.is_finite()) {
        return;
    }
    let width = width.clamp(1., 64.);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut first, mut last) = (0_f64, 1_f64);
    for (p, q) in [
        (-dx, a.0 + width),
        (dx, image.width() as f64 + width - a.0),
        (-dy, a.1 + width),
        (dy, image.height() as f64 + width - a.1),
    ] {
        if p == 0. {
            if q < 0. {
                return;
            }
        } else if p < 0. {
            first = first.max(q / p);
        } else {
            last = last.min(q / p);
        }
    }
    if first > last {
        return;
    }
    let b = (a.0 + dx * last, a.1 + dy * last);
    let a = (a.0 + dx * first, a.1 + dy * first);
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().max(1.) as usize;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        disc(
            image,
            (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t),
            width / 2.,
            c,
        );
    }
}
fn polygon(image: &mut RgbImage, pts: &[(f64, f64)], c: [u8; 3]) {
    if pts.len() < 3 {
        return;
    }
    let y0 = pts
        .iter()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min)
        .max(0.) as u32;
    let y1 = pts
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .min(image.height() as f64 - 1.) as u32;
    for y in y0..=y1 {
        let scan = y as f64 + 0.5;
        let mut xs = vec![];
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if (a.1 <= scan && b.1 > scan) || (b.1 <= scan && a.1 > scan) {
                xs.push(a.0 + (scan - a.1) * (b.0 - a.0) / (b.1 - a.1));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.as_chunks::<2>().0 {
            let x0 = pair[0].max(0.).ceil() as u32;
            let x1 = pair[1].min(image.width() as f64 - 1.).floor() as u32;
            for x in x0..=x1 {
                image.put_pixel(x, y, Rgb(c));
            }
        }
    }
}
