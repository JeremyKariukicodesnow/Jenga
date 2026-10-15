//! Original dot lettering and outlined icons; no proprietary UI font assets.
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
pub fn brand(ui: &mut egui::Ui, text: &str, scale: f32) {
    let step = 3.0 * scale;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(text.chars().count() as f32 * 6. * step, 8. * step),
        egui::Sense::hover(),
    );
    for (i, ch) in text.to_uppercase().chars().enumerate() {
        let rows = glyph(ch);
        for (y, bits) in rows.into_iter().enumerate() {
            for x in 0..5 {
                if bits & (1 << (4 - x)) != 0 {
                    ui.painter().circle_filled(
                        rect.min
                            + Vec2::new(
                                (i as f32 * 6. + x as f32 + 0.5) * step,
                                (y as f32 + 0.5) * step,
                            ),
                        0.95 * scale,
                        Color32::from_rgb(24, 29, 36),
                    );
                }
            }
        }
    }
}
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '.' => [0, 0, 0, 0, 0, 6, 6],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        _ => [0; 7],
    }
}
/// Slow architectural rings and a subtle dot field, restricted to interface surfaces.
pub fn animated_brand(ui: &mut egui::Ui, text: &str, scale: f32) {
    let step = 3. * scale;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(text.len() as f32 * 6. * step, 8. * step),
        egui::Sense::hover(),
    );
    let time = ui.input(|i| i.time) as f32;
    let phase = (time + text.len() as f32 * 0.31).rem_euclid(9.);
    let reveal = if phase < 2.4 {
        phase / 2.4
    } else if phase < 6.5 {
        1.
    } else if phase < 8.2 {
        (8.2 - phase) / 1.7
    } else {
        0.
    };
    let mut dots = vec![];
    for (i, ch) in text.to_uppercase().chars().enumerate() {
        for (y, bits) in glyph(ch).into_iter().enumerate() {
            for x in 0..5 {
                if bits & (1 << (4 - x)) != 0 {
                    dots.push((i, x, y));
                }
            }
        }
    }
    for (j, (i, x, y)) in dots.iter().enumerate() {
        let alpha = (reveal * dots.len() as f32 - j as f32).clamp(0., 1.);
        let pos = rect.min
            + Vec2::new(
                (*i as f32 * 6. + *x as f32 + 0.5) * step,
                (*y as f32 + 0.5) * step,
            )
            + Vec2::new(0., (time * 2. + j as f32 * 0.08).sin() * scale * 0.35);
        if alpha > 0. {
            ui.painter().circle_filled(
                pos,
                0.95 * scale,
                Color32::from_rgba_unmultiplied(24, 29, 36, (alpha * 255.) as u8),
            );
        }
    }
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));
}
pub fn pattern(ui: &egui::Ui, rect: egui::Rect) {
    let time = ui.input(|i| i.time) as f32;
    let p = ui.painter().with_clip_rect(rect);
    // Project/Home depth motif: rotating wireframe blocks, original geometry.
    for block in 0..3 {
        let center =
            rect.right_top() + Vec2::new(-90. - block as f32 * 74., 95. + block as f32 * 20.);
        let yaw = time * 0.22 + block as f32;
        let pitch: f32 = 0.65;
        let project = |x: f32, y: f32, z: f32| {
            let xx = x * yaw.cos() - z * yaw.sin();
            let zz = x * yaw.sin() + z * yaw.cos();
            center + Vec2::new(xx * 22., (y * pitch.cos() - zz * pitch.sin()) * 22.)
        };
        let vertices: Vec<_> = (0..8)
            .map(|i| {
                project(
                    if i & 1 == 0 { -1. } else { 1. },
                    if i & 2 == 0 { -1. } else { 1. },
                    if i & 4 == 0 { -1. } else { 1. },
                )
            })
            .collect();
        for i in 0..8 {
            for bit in [1, 2, 4] {
                let j = i ^ bit;
                if j > i {
                    p.line_segment(
                        [vertices[i], vertices[j]],
                        Stroke::new(0.8_f32, Color32::from_black_alpha(32)),
                    );
                }
            }
        }
        p.circle_filled(
            vertices[((time * 1.1) as usize + block) % 8],
            2.1,
            Color32::from_rgb(225, 35, 43),
        );
    }
    // Distributed waves and travelling nodes across the whole Home surface.
    let columns = (rect.width() / 26.).ceil() as i32;
    let rows = (rect.height() / 26.).ceil() as i32;
    for x in 0..columns {
        for y in 0..rows {
            let pulse = (time * 1.1 + x as f32 * 0.29 - y as f32 * 0.21).sin();
            let pos = rect.min + Vec2::new(x as f32 * 26. + 8., y as f32 * 26. + 8.);
            p.circle_filled(
                pos,
                0.6 + 0.3 * pulse,
                Color32::from_gray((234. + pulse * 8.) as u8),
            );
        }
    }
    for lane in 0..5 {
        let y = rect.top() + rect.height() * (0.25 + lane as f32 * 0.15);
        let x = rect.left() + 20.;
        let end = rect.right() - 20.;
        p.line_segment(
            [Pos2::new(x, y), Pos2::new(end, y)],
            Stroke::new(0.5_f32, Color32::from_gray(235)),
        );
        let travel = (time * 0.09 + lane as f32 * 0.21).fract();
        p.circle_filled(
            Pos2::new(x + (end - x) * travel, y),
            2.,
            Color32::from_rgba_unmultiplied(225, 35, 43, 100),
        );
    }
    let origin = rect.right_top() + Vec2::new(-75., 35.);
    for x in 0..24 {
        for y in 0..15 {
            let pos = rect.right_top() + Vec2::new(-380. + x as f32 * 16., y as f32 * 16.);
            let wave = (time * 1.5 + x as f32 * 0.23 + y as f32 * 0.2).sin();
            p.circle_filled(
                pos,
                0.65 + 0.3 * (wave + 1.),
                Color32::from_gray((210. + wave * 15.) as u8),
            );
        }
    }
    for ring in 0..5 {
        let radius = 50. + ring as f32 * 24.;
        let start = time * 0.16 + ring as f32 * 0.2;
        let points = (0..80)
            .map(|i| {
                let a = start + i as f32 * 0.052;
                origin + Vec2::new(a.cos(), a.sin()) * radius
            })
            .collect();
        p.add(egui::Shape::line(
            points,
            Stroke::new(0.7_f32, Color32::from_gray(204)),
        ));
        p.circle_filled(
            origin + Vec2::new(start.cos(), start.sin()) * radius,
            2.,
            Color32::from_rgb(225, 35, 43),
        );
    }
    for row in 0..4 {
        let y = rect.top() + 200. + row as f32 * 20.;
        let x = rect.right() - 440. + row as f32 * 18.;
        let trace = vec![
            Pos2::new(x, y),
            Pos2::new(x + 90., y),
            Pos2::new(x + 110., y - 20.),
            Pos2::new(rect.right() - 35., y - 20.),
        ];
        p.add(egui::Shape::line(
            trace,
            Stroke::new(0.6_f32, Color32::from_gray(223)),
        ));
        let t = (time * 0.12 + row as f32 * 0.2).fract();
        let pos = if t < 0.4 {
            Pos2::new(x + t / 0.4 * 90., y)
        } else if t < 0.5 {
            Pos2::new(x + 90. + (t - 0.4) / 0.1 * 20., y - (t - 0.4) / 0.1 * 20.)
        } else {
            Pos2::new(
                x + 110. + (t - 0.5) / 0.5 * (rect.right() - 35. - x - 110.),
                y - 20.,
            )
        };
        p.circle_filled(pos, 1.6, Color32::from_rgb(225, 35, 43));
    }
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(40));
}
pub fn loading(ui: &mut egui::Ui, logo: &egui::TextureHandle) {
    let time = ui.input(|i| i.time) as f32;
    let size = 32. + (time * 2.).sin() * 2.;
    ui.horizontal(|ui| {
        ui.add(
            egui::Image::new((logo.id(), Vec2::splat(size)))
                .rotate((time * 1.5).sin() * 0.07, Vec2::splat(0.5)),
        );
        brand(ui, "JENGA", 0.7);
        ui.label("Loading libraries…");
    });
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));
}
pub fn button(ui: &mut egui::Ui, label: &str, icon: &str, selected: bool) -> egui::Response {
    let width = (label.chars().count() as f32 * 6.7 + 16.).max(58.);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 47.), egui::Sense::click());
    if selected || response.hovered() {
        ui.painter()
            .rect_filled(rect, 5., Color32::from_rgb(249, 221, 217));
    }
    let c = if selected {
        Color32::from_rgb(225, 35, 43)
    } else {
        Color32::from_rgb(30, 35, 44)
    };
    let p = ui.painter();
    let center = rect.center_top() + Vec2::new(0., 15.);
    let st = Stroke::new(1.6_f32, c);
    let point = |x, y| center + Vec2::new(x, y);
    let line = |a: Pos2, b: Pos2| {
        p.line_segment([a, b], st);
    };
    match icon {
        "delete" => {
            line(point(-8., -5.), point(8., -5.));
            line(point(-3., -9.), point(3., -9.));
            line(point(-6., -5.), point(-5., 9.));
            line(point(6., -5.), point(5., 9.));
            line(point(-5., 9.), point(5., 9.));
            line(point(-2., -2.), point(-2., 6.));
            line(point(2., -2.), point(2., 6.));
        }
        "rename" => {
            line(point(-8., 8.), point(6., -8.));
            line(point(-5., 10.), point(9., -6.));
            line(point(6., -8.), point(9., -6.));
            line(point(-8., 8.), point(-9., 11.));
            line(point(-9., 11.), point(-5., 10.));
        }
        "home" => {
            p.add(egui::Shape::line(
                vec![point(-11., 0.), point(0., -10.), point(11., 0.)],
                st,
            ));
            line(point(-8., -2.), point(-8., 10.));
            line(point(8., -2.), point(8., 10.));
            line(point(-8., 10.), point(8., 10.));
            line(point(-2., 10.), point(-2., 3.));
            line(point(-2., 3.), point(3., 3.));
            line(point(3., 3.), point(3., 10.));
        }
        "project" => {
            p.add(egui::Shape::closed_line(
                vec![
                    point(-11., -7.),
                    point(-3., -7.),
                    point(0., -3.),
                    point(11., -3.),
                    point(11., 9.),
                    point(-11., 9.),
                ],
                st,
            ));
        }
        "archive" => {
            p.rect_stroke(
                egui::Rect::from_center_size(center, Vec2::new(21., 16.)),
                2.,
                st,
                egui::StrokeKind::Inside,
            );
            line(point(-10., -3.), point(10., -3.));
            line(point(-3., 1.), point(3., 1.));
        }
        "save_as" => {
            p.rect_stroke(
                egui::Rect::from_center_size(point(-3., -2.), Vec2::new(15., 17.)),
                1.,
                st,
                egui::StrokeKind::Inside,
            );
            line(point(1., 6.), point(10., 6.));
            line(point(6., 2.), point(10., 6.));
            line(point(6., 10.), point(10., 6.));
        }
        "wire" => {
            line(point(-10., 7.), point(-3., -5.));
            line(point(-3., -5.), point(10., -5.));
            for q in [point(-10., 7.), point(10., -5.)] {
                p.circle_stroke(q, 2., st);
            }
        }
        "symbol" => {
            p.rect_stroke(
                egui::Rect::from_center_size(center, Vec2::splat(17.)),
                1.,
                st,
                egui::StrokeKind::Inside,
            );
            line(point(-5., 0.), point(5., 0.));
            line(point(0., -5.), point(0., 5.));
        }
        "save" => {
            p.rect_stroke(
                egui::Rect::from_center_size(center, Vec2::splat(20.)),
                1.,
                st,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                egui::Rect::from_center_size(point(0., 4.), Vec2::new(12., 9.)),
                0.,
                st,
                egui::StrokeKind::Inside,
            );
            line(point(-5., -9.), point(-5., -3.));
            line(point(-5., -3.), point(5., -3.));
        }
        "select" => {
            p.add(egui::Shape::closed_line(
                vec![
                    point(-6., -10.),
                    point(-6., 9.),
                    point(-1., 4.),
                    point(5., 10.),
                    point(8., 7.),
                    point(2., 1.),
                    point(10., 0.),
                ],
                st,
            ));
        }
        "erc" => {
            p.circle_stroke(center, 10., st);
            line(point(-5., 0.), point(-1., 5.));
            line(point(-1., 5.), point(6., -4.));
        }
        "export" => {
            line(point(0., 7.), point(0., -10.));
            line(point(0., -10.), point(-5., -5.));
            line(point(0., -10.), point(5., -5.));
            line(point(-9., 0.), point(-9., 10.));
            line(point(-9., 10.), point(9., 10.));
            line(point(9., 10.), point(9., 0.));
        }
        "fit" => {
            for (x, y) in [(-1., -1.), (1., -1.), (-1., 1.), (1., 1.)] {
                line(point(x * 10., y * 5.), point(x * 10., y * 10.));
                line(point(x * 10., y * 10.), point(x * 5., y * 10.));
            }
        }
        "undo" | "redo" => {
            let sign = if icon == "undo" { -1. } else { 1. };
            line(point(sign * 9., -4.), point(-sign * 5., -4.));
            line(point(-sign * 5., -4.), point(-sign * 9., 2.));
            line(point(sign * 9., -4.), point(sign * 3., -9.));
            line(point(sign * 9., -4.), point(sign * 3., 1.));
        }
        _ => {
            p.circle_stroke(center, 8., st);
            line(point(-4., 0.), point(4., 0.));
            if icon == "plus" {
                line(point(0., -4.), point(0., 4.));
            }
        }
    }
    p.text(
        rect.center_bottom() - Vec2::new(0., 3.),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::monospace(11.),
        c,
    );
    response
}
