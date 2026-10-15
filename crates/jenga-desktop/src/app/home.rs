use super::*;
impl App {
    pub(super) fn home(&mut self, ui: &mut egui::Ui) {
        crate::design::pattern(ui, ui.max_rect());
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(24.);
            crate::design::animated_brand(ui, "JENGA", 2.4);
            crate::design::brand(ui, "YOUR WORKSPACE", 1.1);
            ui.label(
                RichText::new("Create.  Connect.  Build.")
                    .monospace()
                    .size(16.),
            );
            ui.add_space(20.);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [160., 38.],
                        egui::Button::new(RichText::new("+  New project").color(Color32::WHITE))
                            .fill(Color32::from_gray(24)),
                    )
                    .clicked()
                {
                    self.new_project = true;
                }
                if ui
                    .add_sized([160., 38.], egui::Button::new("Open project…"))
                    .clicked()
                {
                    self.open();
                }
                if ui.button("Libraries").clicked() {
                    self.library_dialog = true;
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Connect Assistant →").clicked() {
                    self.connect_dialog = true;
                }
                if ui.button("Explore features / Help").clicked() {
                    self.help_dialog = true;
                }
            });
            ui.add_space(18.);
            ui.add(
                egui::TextEdit::singleline(&mut self.project_search)
                    .hint_text("Search projects…")
                    .desired_width(ui.available_width().min(720.)),
            );
            ui.add_space(20.);
            crate::design::brand(ui, "RECENT PROJECTS", 0.85);
            ui.add_space(12.);
            let paths: Vec<_> = self
                .settings
                .recent
                .iter()
                .filter(|p| {
                    p.to_string_lossy()
                        .to_lowercase()
                        .contains(&self.project_search.to_lowercase())
                })
                .cloned()
                .collect();
            if paths.is_empty() {
                ui.label("Open a KiCad project or create your first schematic.");
            }
            let columns = ((ui.available_width() + 12.) / 280.).floor().clamp(1., 4.) as usize;
            let width =
                ((ui.available_width() - 12. * (columns - 1) as f32) / columns as f32).min(360.);
            for row in paths.chunks(columns) {
                ui.horizontal(|ui| {
                    for path in row {
                        let modified = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
                        if self
                            .project_previews
                            .get(path)
                            .is_none_or(|(time, _)| *time != modified)
                            && let Ok(doc) = Document::open(path)
                        {
                            self.project_previews
                                .insert(path.clone(), (modified, Scene::from_document(&doc)));
                        }
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(width, 210.), egui::Sense::click());
                        let progress = ui.ctx().animate_bool_with_time(
                            response.id.with("lift"),
                            response.hovered(),
                            0.18,
                        );
                        let card = rect.translate(Vec2::new(0., -4. * progress));
                        let p = ui.painter();
                        p.rect_filled(
                            card.translate(Vec2::new(0., 3. + 3. * progress)),
                            8.,
                            Color32::from_black_alpha((10. + progress * 12.) as u8),
                        );
                        p.rect_filled(card, 8., Color32::from_rgb(253, 252, 248));
                        p.rect_stroke(
                            card,
                            8.,
                            Stroke::new(
                                1_f32,
                                if response.hovered() {
                                    ACCENT.gamma_multiply(0.45)
                                } else {
                                    Color32::from_gray(211)
                                },
                            ),
                            egui::StrokeKind::Inside,
                        );
                        let preview = egui::Rect::from_min_max(
                            card.min + Vec2::splat(10.),
                            card.right_top() + Vec2::new(-10., 145.),
                        );
                        if let Some((_, scene)) = self.project_previews.get(path) {
                            let mut view = View {
                                show_grid: false,
                                ..View::default()
                            };
                            view.fit(scene, preview);
                            canvas::paint(
                                &ui.painter_at(preview),
                                &view,
                                preview,
                                scene,
                                None,
                                false,
                            );
                        } else {
                            p.text(
                                preview.center(),
                                egui::Align2::CENTER_CENTER,
                                "Preview unavailable",
                                egui::FontId::monospace(12.),
                                Color32::GRAY,
                            );
                        }
                        let name = path.file_stem().unwrap_or_default().to_string_lossy();
                        let name = p.layout(
                            name.into_owned(),
                            egui::FontId::monospace(15.),
                            Color32::from_gray(27),
                            width - 24.,
                        );
                        p.galley(
                            card.left_top() + Vec2::new(12., 153.),
                            name,
                            Color32::from_gray(27),
                        );
                        let age = modified
                            .and_then(|t| t.elapsed().ok())
                            .map(|s| {
                                if s.as_secs() < 3600 {
                                    format!("Modified {} min ago", s.as_secs() / 60)
                                } else if s.as_secs() < 86400 {
                                    format!("Modified {} hours ago", s.as_secs() / 3600)
                                } else {
                                    format!("Modified {} days ago", s.as_secs() / 86400)
                                }
                            })
                            .unwrap_or_else(|| "File unavailable".into());
                        p.text(
                            card.left_bottom() + Vec2::new(12., -15.),
                            egui::Align2::LEFT_CENTER,
                            age,
                            egui::FontId::monospace(11.),
                            Color32::from_gray(105),
                        );
                        if response.on_hover_text(path.display().to_string()).clicked()
                            && path.exists()
                        {
                            self.open_path(path);
                        }
                    }
                });
                ui.add_space(12.);
            }
            ui.add_space(12.);
            crate::design::brand(ui, "ALL PROJECTS", 0.85);
            if self.home_projects {
                ui.scroll_to_cursor(Some(egui::Align::TOP));
                self.home_projects = false;
            }
            ui.add_space(8.);
            for path in &paths {
                let label = format!(
                    "□   {}",
                    path.file_stem().unwrap_or_default().to_string_lossy()
                );
                if ui
                    .add_sized(
                        [ui.available_width(), 29.],
                        egui::Button::new(label).truncate(),
                    )
                    .on_hover_text(path.display().to_string())
                    .clicked()
                    && path.exists()
                {
                    self.open_path(path);
                }
            }
            ui.add_space(20.);
            ui.separator();
            ui.add_space(12.);
            if let Some(libs) = &self.libraries {
                ui.label(format!(
                    "{} symbols · {} footprints · {} library sources",
                    libs.entries.len(),
                    self.footprint_ids.len(),
                    libs.sources.len()
                ));
            } else {
                crate::design::loading(ui, &self.logo);
            }
            ui.label(
                RichText::new("KiCad compatible · Rust · Open source")
                    .small()
                    .weak(),
            );
            ui.add_space(20.);
        });
    }
}
