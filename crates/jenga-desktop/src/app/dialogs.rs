use super::*;
impl App {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        if self.new_project {
            let mut open = true;
            egui::Window::new("Create project")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Project name");
                    ui.text_edit_singleline(&mut self.project_name);
                    ui.add_space(8.);
                    ui.label("Parent folder");
                    ui.horizontal(|ui| {
                        ui.label(self.project_folder.display().to_string());
                        if ui.button("Choose…").clicked()
                            && let Some(folder) = rfd::FileDialog::new()
                                .set_directory(&self.project_folder)
                                .pick_folder()
                        {
                            self.project_folder = folder;
                        }
                    });
                    ui.add_space(12.);
                    if ui.button("Create project and schematic").clicked() {
                        match create_project(&self.project_folder, &self.project_name) {
                            Ok(doc) => {
                                self.set_doc(doc);
                                self.new_project = false;
                            }
                            Err(e) => self.fail(e),
                        }
                    }
                });
            self.new_project &= open;
        }
        if self.text_dialog {
            let mut open = true;
            egui::Window::new(if self.tool == Tool::Label {
                "Place net label"
            } else {
                "Place text"
            })
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.text_edit_multiline(&mut self.text);
                if ui
                    .add_enabled(
                        !self.text.trim().is_empty(),
                        egui::Button::new("Place on canvas"),
                    )
                    .clicked()
                {
                    self.text_dialog = false;
                }
            });
            if !open {
                self.text_dialog = false;
                self.tool = Tool::Select;
            }
        }
        if self.chooser {
            let mut open = true;
            egui::Window::new("Choose symbol")
                .open(&mut open)
                .default_size([780., 560.])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Search");
                        ui.text_edit_singleline(&mut self.search);
                    });
                    ui.separator();
                    let needle = self.search.to_lowercase();
                    let results: Vec<_> = self
                        .libraries
                        .as_ref()
                        .map(|l| {
                            l.entries
                                .iter()
                                .filter(|e| e.id.to_lowercase().contains(&needle))
                                .take(350)
                                .map(|e| (e.id.clone(), e.source.clone()))
                                .collect()
                        })
                        .unwrap_or_default();
                    ui.columns(2, |columns| {
                        egui::ScrollArea::vertical()
                            .max_height(420.)
                            .show(&mut columns[0], |ui| {
                                for (id, source) in &results {
                                    if ui
                                        .selectable_label(self.chosen_id.as_ref() == Some(id), id)
                                        .on_hover_text(source)
                                        .clicked()
                                    {
                                        match self.libraries.as_mut().unwrap().symbol(id) {
                                            Ok(def) => {
                                                self.chosen_id = Some(id.clone());
                                                self.chosen_symbol = Some(def);
                                                self.unit = 1;
                                            }
                                            Err(e) => self.fail(e),
                                        }
                                    }
                                }
                            });
                        columns[1].label(self.chosen_id.as_deref().unwrap_or("Select a symbol"));
                        if let Some(def) = &self.chosen_symbol {
                            let scene = Scene::symbol_preview(def);
                            let (rect, _) = columns[1].allocate_exact_size(
                                Vec2::new(columns[1].available_width(), 260.),
                                egui::Sense::hover(),
                            );
                            let mut view = View {
                                show_grid: false,
                                ..View::default()
                            };
                            view.fit(&scene, rect);
                            canvas::paint(
                                &columns[1].painter_at(rect),
                                &view,
                                rect,
                                &scene,
                                None,
                                false,
                            );
                            let max_unit = def
                                .children("symbol")
                                .filter_map(|s| s.value(1).rsplit('_').nth(1)?.parse::<u32>().ok())
                                .max()
                                .unwrap_or(1)
                                .max(1);
                            columns[1].horizontal(|ui| {
                                ui.label("Unit");
                                ui.add(egui::DragValue::new(&mut self.unit).range(1..=max_unit));
                            });
                            columns[1].label(property(def, "Description"));
                            columns[1].label(property(def, "Footprint"));
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} matches shown (search to narrow)",
                            results.len()
                        ));
                        if ui
                            .add_enabled(
                                self.chosen_symbol.is_some() && self.doc.is_some(),
                                egui::Button::new("Place symbol"),
                            )
                            .clicked()
                        {
                            self.chooser = false;
                            self.tool = Tool::Symbol;
                        }
                    });
                });
            if !open {
                self.chooser = false;
                self.tool = Tool::Select;
            }
        }
        if self.library_dialog {
            let mut open = true;
            egui::Window::new("Library sources").open(&mut open).default_size([800.,480.]).show(ctx,|ui|{
            ui.label("Priority: user-added → project → KiCad tables → installed KiCad → Jenga bundle.");ui.label("Schematic-embedded symbols always determine existing designs. First matching library ID wins.");ui.separator();
            egui::ScrollArea::vertical().max_height(290.).show(ui,|ui|{if let Some(libs)=&self.libraries{for source in &libs.sources{ui.horizontal(|ui|{ui.label(RichText::new(&source.name).color(ACCENT));ui.label(&source.kind);});ui.label(RichText::new(source.path.display().to_string()).small().weak());ui.separator();}for warning in &libs.warnings{ui.label(RichText::new(warning).color(Color32::YELLOW));}}else{ui.spinner();}});
            ui.horizontal(|ui|{if ui.button("Add symbol library…").clicked()&& let Some(path)=rfd::FileDialog::new().add_filter("KiCad symbol library",&["kicad_sym"]).pick_file(){let name=path.file_stem().unwrap_or_default().to_string_lossy().into_owned();self.settings.libraries.insert(0,Source{name:format!("User / {name}"),path,kind:"symbols".into()});self.persist();self.reload_libraries();}
if ui.button("Add footprint folder…").clicked()&& let Some(path)=rfd::FileDialog::new().pick_folder(){self.settings.libraries.insert(0,Source{name:"User footprints".into(),path,kind:"footprints".into()});self.persist();self.reload_libraries();}
if ui.button("Rescan").clicked(){self.reload_libraries();}});
            let extras=self.settings.libraries.clone();for(index,source)in extras.iter().enumerate(){ui.horizontal(|ui|{ui.label(&source.name);if ui.small_button("Remove user source").clicked(){self.settings.libraries.remove(index);self.persist();self.reload_libraries();}});}
            ui.add_space(8.);ui.label("Footprints are bundled for future PCB work; 3D models are not included.");
        });
            self.library_dialog = open;
        }
        if self.show_checks {
            let mut open = true;
            egui::Window::new("Electrical rules · basic single-sheet checks").open(&mut open).default_size([680.,380.]).show(ctx,|ui|{ui.label("Checks include unconnected pins, conflicting outputs, duplicate references and missing power drivers. This is not the complete KiCad ERC.");ui.separator();if self.checks.is_empty(){ui.label("No issues found by the implemented checks.");}let issues=self.checks.clone();egui::ScrollArea::vertical().show(ui,|ui|{for issue in issues{if ui.button(format!("{} · {}",issue.severity,issue.message)).clicked(){self.selected=issue.object;if let Some(i)=issue.object{let scene=Scene::from_document(self.doc.as_ref().unwrap());if let Some((a,b))=scene.object_bounds(i){self.view.center=((a.0+b.0)/2.,(a.1+b.1)/2.);self.view.scale=12.;}}}}});});
            self.show_checks = open;
        }
        if self.about {
            egui::Window::new("About Jenga").open(&mut self.about).show(ctx,|ui|{ui.heading(format!("Jenga {}",env!("CARGO_PKG_VERSION")));ui.label("Open-source Rust schematic editor · GPL-3.0-or-later");ui.label("A: symbol · W: wire · J: junction · L: label · T: text\nQ: no-connect · M: move · R: rotate · Delete: remove\nHome: fit · Esc: cancel · Ctrl+S: save · Ctrl+Z: undo");ui.separator();ui.label("Install a newer Jenga Setup.exe to upgrade in place. Projects and settings are kept outside the installation folder. Automatic online updates are not configured.");ui.label("Modern KiCad files preserve unknown data. Legacy .sch import, PCB editing and full KiCad ERC are not implemented in 0.1. KiCad NewStroke lettering and embedded symbol geometry are used.");ui.hyperlink_to("License","https://www.gnu.org/licenses/gpl-3.0.html");ui.hyperlink_to("KiCad library attribution","https://www.kicad.org/libraries/license/");});
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new("Jenga · message")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(error);
                    if ui.button("OK").clicked() {
                        self.error = None;
                    }
                });
        }
    }
}
