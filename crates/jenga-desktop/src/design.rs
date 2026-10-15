//! Original dot lettering and outlined icons; no proprietary UI font assets.
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
pub fn brand(ui: &mut egui::Ui, text: &str, scale: f32) {
    let paths = jenga_core::font::paths(text, 16., true);
    let (a, b) = jenga_core::scene::bounds(&paths.iter().flatten().copied().collect::<Vec<_>>());
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new((b.0 - a.0) as f32 * scale + 4., 24. * scale),
        egui::Sense::hover(),
    );
    for path in paths {
        for segment in path.windows(2) {
            let length = (segment[1].0 - segment[0].0).hypot(segment[1].1 - segment[0].1);
            let count = (length / 1.9).ceil().max(1.) as usize;
            for i in 0..=count {
                let t = i as f64 / count as f64;
                let x = segment[0].0 + (segment[1].0 - segment[0].0) * t - a.0;
                let y = segment[0].1 + (segment[1].1 - segment[0].1) * t - a.1;
                ui.painter().circle_filled(
                    rect.min + Vec2::new(x as f32 * scale + 2., y as f32 * scale + 5.),
                    0.65 * scale,
                    Color32::from_rgb(24, 29, 36),
                );
            }
        }
    }
}
pub fn button(ui: &mut egui::Ui, label: &str, icon: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(66., 60.), egui::Sense::click());
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
    let center = rect.center_top() + Vec2::new(0., 20.);
    let st = Stroke::new(1.6_f32, c);
    let point = |x, y| center + Vec2::new(x, y);
    let line = |a: Pos2, b: Pos2| {
        p.line_segment([a, b], st);
    };
    match icon {
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
        rect.center_bottom() - Vec2::new(0., 9.),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::proportional(12.),
        c,
    );
    response
}
