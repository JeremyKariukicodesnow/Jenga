use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape as EShape, Stroke, Vec2};
use jenga_core::scene::{Point, Scene, Shape};

#[derive(Clone)]
pub struct View {
    pub scale: f32,
    pub center: Point,
    pub grid: f64,
    pub show_grid: bool,
}
impl Default for View {
    fn default() -> Self {
        Self {
            scale: 4.,
            center: (148.5, 105.),
            grid: 1.27,
            show_grid: true,
        }
    }
}
impl View {
    pub fn screen(&self, p: Point, rect: Rect) -> Pos2 {
        rect.center()
            + Vec2::new(
                (p.0 - self.center.0) as f32 * self.scale,
                (p.1 - self.center.1) as f32 * self.scale,
            )
    }
    pub fn world(&self, p: Pos2, rect: Rect) -> Point {
        let d = (p - rect.center()) / self.scale;
        (self.center.0 + d.x as f64, self.center.1 + d.y as f64)
    }
    pub fn snap(&self, p: Point) -> Point {
        (
            (p.0 / self.grid).round() * self.grid,
            (p.1 / self.grid).round() * self.grid,
        )
    }
    pub fn fit(&mut self, scene: &Scene, rect: Rect) {
        let (a, b) = scene.content_bounds();
        self.center = ((a.0 + b.0) / 2., (a.1 + b.1) / 2.);
        self.scale = (rect.width() / ((b.0 - a.0 + 20.) as f32))
            .min(rect.height() / ((b.1 - a.1 + 20.) as f32))
            .clamp(0.5, 30.);
    }
}
pub fn color(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
pub fn paint(
    painter: &egui::Painter,
    view: &View,
    rect: Rect,
    scene: &Scene,
    selected: Option<usize>,
    page: bool,
) {
    painter.rect_filled(rect, 0., Color32::from_rgb(243, 242, 237));
    if page {
        let page_rect =
            Rect::from_two_pos(view.screen((0., 0.), rect), view.screen((297., 210.), rect));
        painter.rect_filled(page_rect, 0., Color32::from_rgb(255, 255, 250));
        painter.rect_stroke(
            page_rect,
            0.,
            Stroke::new(1.0_f32, Color32::from_gray(180)),
            egui::StrokeKind::Inside,
        );
        let border = Rect::from_two_pos(
            view.screen((10., 10.), rect),
            view.screen((287., 200.), rect),
        );
        painter.rect_stroke(
            border,
            0.,
            Stroke::new(0.8_f32, Color32::from_rgb(140, 28, 25)),
            egui::StrokeKind::Inside,
        );
        let title = Rect::from_two_pos(
            view.screen((215., 182.), rect),
            view.screen((287., 200.), rect),
        );
        painter.rect_stroke(
            title,
            0.,
            Stroke::new(0.8_f32, Color32::from_rgb(140, 28, 25)),
            egui::StrokeKind::Inside,
        );
        painter.text(
            view.screen((217., 185.), rect),
            Align2::LEFT_TOP,
            "Jenga · Schematic",
            FontId::proportional((1.5 * view.scale).max(8.)),
            Color32::from_rgb(140, 28, 25),
        );
    }
    if view.show_grid && view.grid * view.scale as f64 >= 5. {
        let a = view.world(rect.min, rect);
        let b = view.world(rect.max, rect);
        let step = view.grid;
        let x0 = (a.0 / step).floor() as i32;
        let x1 = (b.0 / step).ceil() as i32;
        let y0 = (a.1 / step).floor() as i32;
        let y1 = (b.1 / step).ceil() as i32;
        for x in x0..=x1 {
            for y in y0..=y1 {
                painter.circle_filled(
                    view.screen((x as f64 * step, y as f64 * step), rect),
                    0.65,
                    Color32::from_gray(197),
                );
            }
        }
    }
    for draw in &scene.draws {
        match &draw.shape {
            Shape::Path {
                points,
                closed,
                fill,
                color: c,
                width,
            } => {
                let points: Vec<_> = points.iter().map(|p| view.screen(*p, rect)).collect();
                let stroke = Stroke::new((*width as f32 * view.scale).max(0.8), color(*c));
                if *closed {
                    if let Some(c) = fill {
                        painter.add(EShape::convex_polygon(
                            points.clone(),
                            color(*c),
                            Stroke::NONE,
                        ));
                    }
                    painter.add(EShape::closed_line(points, stroke));
                } else {
                    painter.add(EShape::line(points, stroke));
                }
            }
            Shape::Circle {
                center,
                radius,
                fill,
                color: c,
                width,
            } => {
                painter.circle(
                    view.screen(*center, rect),
                    *radius as f32 * view.scale,
                    fill.map(color).unwrap_or(Color32::TRANSPARENT),
                    Stroke::new((*width as f32 * view.scale).max(0.8), color(*c)),
                );
            }
            Shape::Text {
                at,
                text,
                size,
                angle,
                color: c,
                left,
            } => {
                let font = FontId::proportional((*size as f32 * view.scale).max(1.));
                let galley = painter.layout_no_wrap(text.clone(), font, color(*c));
                let pos = view.screen(*at, rect);
                let a = -(*angle as f32).to_radians();
                let anchor = if *left {
                    Vec2::new(0., galley.size().y / 2.)
                } else {
                    galley.size() / 2.
                };
                let rotated = Vec2::new(
                    anchor.x * a.cos() - anchor.y * a.sin(),
                    anchor.x * a.sin() + anchor.y * a.cos(),
                );
                let mut shape = egui::epaint::TextShape::new(pos - rotated, galley, color(*c));
                shape.angle = a;
                painter.add(shape);
            }
        }
    }
    if let Some(index) = selected
        && let Some((a, b)) = scene.object_bounds(index)
    {
        let box_rect = Rect::from_two_pos(view.screen(a, rect), view.screen(b, rect)).expand(5.);
        painter.rect_stroke(
            box_rect,
            2.,
            Stroke::new(1.5_f32, Color32::from_rgb(30, 144, 205)),
            egui::StrokeKind::Outside,
        );
    }
}
pub fn hit(scene: &Scene, view: &View, p: Point) -> Option<usize> {
    let tolerance = 4.0 / view.scale as f64;
    let mut found = None;
    let mut best = f64::INFINITY;
    for draw in &scene.draws {
        let Some(index) = draw.object else {
            continue;
        };
        let score = match &draw.shape {
            Shape::Path { points, .. } => points
                .windows(2)
                .map(|w| distance_segment(p, w[0], w[1]))
                .fold(f64::INFINITY, f64::min),
            Shape::Circle { center, radius, .. } => {
                (((p.0 - center.0).powi(2) + (p.1 - center.1).powi(2)).sqrt() - radius).abs()
            }
            Shape::Text { .. } => {
                let (a, b) = draw.shape.bounds();
                if p.0 >= a.0 && p.0 <= b.0 && p.1 >= a.1 && p.1 <= b.1 {
                    0.
                } else {
                    f64::INFINITY
                }
            }
        };
        if score < tolerance && score < best {
            found = Some(index);
            best = score;
        }
    }
    // Interior selection of a symbol is convenient even when its outline is hollow.
    if found.is_none() {
        for pin in &scene.pins {
            if let Some((a, b)) = scene.object_bounds(pin.object)
                && p.0 >= a.0
                && p.0 <= b.0
                && p.1 >= a.1
                && p.1 <= b.1
            {
                found = Some(pin.object);
                break;
            }
        }
    }
    found
}
fn distance_segment(p: Point, a: Point, b: Point) -> f64 {
    let d = (b.0 - a.0, b.1 - a.1);
    let len = d.0 * d.0 + d.1 * d.1;
    if len < 1e-12 {
        return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt();
    }
    let t = (((p.0 - a.0) * d.0 + (p.1 - a.1) * d.1) / len).clamp(0., 1.);
    ((p.0 - a.0 - t * d.0).powi(2) + (p.1 - a.1 - t * d.1).powi(2)).sqrt()
}
