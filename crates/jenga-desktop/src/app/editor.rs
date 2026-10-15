use super::*;
impl App {
    pub(super) fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            crate::design::brand(ui, "JENGA", 1.2);
            ui.separator();
            ui.menu_button("File", |ui| {
                if ui.button("New project…    Ctrl+N").clicked() {
                    if self.may_replace() {
                        self.new_project = true;
                    }
                    ui.close();
                }
                if ui.button("New schematic").clicked() {
                    if self.may_replace() {
                        self.set_doc(Document::new());
                    }
                    ui.close();
                }
                if ui.button("Open…                 Ctrl+O").clicked() {
                    self.open();
                    ui.close();
                }
                if ui.button("Save                      Ctrl+S").clicked() {
                    self.save(false);
                    ui.close();
                }
                if ui.button("Save as…       Ctrl+Shift+S").clicked() {
                    self.save(true);
                    ui.close();
                }
                ui.separator();
                for kind in ["svg", "pdf", "csv"] {
                    if ui
                        .button(format!("Export {}…", kind.to_uppercase()))
                        .clicked()
                    {
                        self.output(kind);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Project home").clicked() {
                    if self.may_replace() {
                        self.doc = None;
                        self.selected = None;
                    }
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Undo   Ctrl+Z").clicked() {
                    self.undo();
                    ui.close();
                }
                if ui.button("Redo   Ctrl+Y").clicked() {
                    self.redo();
                    ui.close();
                }
                if ui.button("Rotate   R").clicked() {
                    if let Some(i) = self.selected {
                        self.checkpoint();
                        self.doc.as_mut().unwrap().rotate(i);
                    }
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Fit schematic    Home").clicked() {
                    self.fit = true;
                    ui.close();
                }
                ui.checkbox(&mut self.view.show_grid, "Show grid");
            });
            ui.menu_button("Place", |ui| {
                for (label, tool) in [
                    ("Symbol   A", Tool::Symbol),
                    ("Wire   W", Tool::Wire),
                    ("Junction   J", Tool::Junction),
                    ("Label   L", Tool::Label),
                    ("Text   T", Tool::Text),
                    ("No-connect   Q", Tool::NoConnect),
                ] {
                    if ui.button(label).clicked() {
                        self.choose_tool(tool);
                        ui.close();
                    }
                }
            });
            ui.menu_button("Inspect", |ui| {
                if ui.button("Electrical rules check").clicked() {
                    self.inspect();
                    ui.close();
                }
            });
            ui.menu_button("Preferences", |ui| {
                if ui.button("Library sources…").clicked() {
                    self.library_dialog = true;
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About & shortcuts").clicked() {
                    self.about = true;
                    ui.close();
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).weak());
            });
        });
    }
    pub(super) fn home(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui,|ui| {
            ui.add_space(30.);
            ui.horizontal(|ui| {ui.add_space(28.);ui.vertical(|ui| {
                crate::design::brand(ui,"YOUR WORKSPACE",2.1);
                ui.label(RichText::new("Create.  Connect.  Build.").monospace().size(17.).weak());
                ui.add_space(20.);
                ui.horizontal(|ui| {
                    if ui.add_sized([160.,42.],egui::Button::new(RichText::new("+  New project").color(Color32::WHITE)).fill(ACCENT)).clicked(){self.new_project=true;}
                    if ui.add_sized([160.,42.],egui::Button::new("Open project…")).clicked(){self.open();}
                });
                ui.add_space(22.);
                ui.add(egui::TextEdit::singleline(&mut self.project_search).hint_text("Search your projects…").desired_width(600.));
                ui.add_space(22.);crate::design::brand(ui,"RECENT PROJECTS",1.1);ui.add_space(12.);
                let recent=self.settings.recent.clone();
                let filtered:Vec<_>=recent.into_iter().filter(|p|p.to_string_lossy().to_lowercase().contains(&self.project_search.to_lowercase())).collect();
                if filtered.is_empty(){ui.label("Your projects will appear here. Open a KiCad schematic or create a new project.");}
                for row in filtered.chunks(3) {
                    ui.horizontal(|ui| {for path in row {
                        egui::Frame::group(ui.style()).fill(Color32::from_rgb(253,252,248)).inner_margin(12.).show(ui,|ui| {ui.vertical(|ui| {
                            ui.set_width(275.);
                            let modified=std::fs::metadata(path).ok().and_then(|m|m.modified().ok());
                            if self.project_previews.get(path).is_none_or(|(time,_)|*time!=modified) && let Ok(doc)=Document::open(path){self.project_previews.insert(path.clone(),(modified,Scene::from_document(&doc)));}
                            if let Some((_,scene))=self.project_previews.get(path) {
                                let (rect,_)=ui.allocate_exact_size(Vec2::new(275.,140.),egui::Sense::hover());
                                let mut view=View{show_grid:false,..View::default()};view.fit(scene,rect);
                                canvas::paint(&ui.painter_at(rect),&view,rect,scene,None,false);
                            }
                            ui.add_space(8.);
                            let name=path.file_stem().unwrap_or_default().to_string_lossy();
                            if ui.add_enabled(path.exists(),egui::Button::new(RichText::new(name).strong()).min_size(Vec2::new(270.,32.))).clicked(){self.open_path(path);}
                            ui.add(egui::Label::new(RichText::new(path.parent().unwrap_or(path).display().to_string()).small().weak()).truncate()).on_hover_text(path.display().to_string());
                        });});
                    }});ui.add_space(12.);
                }
                ui.add_space(20.);ui.separator();ui.add_space(16.);
                crate::design::brand(ui,"LIBRARY WORKSPACE",1.1);
                if let Some(libs)=&self.libraries {ui.label(format!("{} symbols · {} footprints · {} sources",libs.entries.len(),self.footprint_ids.len(),libs.sources.len()));}
                else {ui.horizontal(|ui|{ui.spinner();ui.label("Discovering installed and bundled libraries…");});}
                ui.label("Official KiCad symbols and footprints included. Your configured KiCad libraries are discovered automatically.");
                if ui.button("Manage libraries…").clicked(){self.library_dialog=true;}
                ui.add_space(15.);ui.label(RichText::new("KiCad compatible · Rust · Open source").small().weak());
            });});
        });
    }
    pub(super) fn palette(&mut self, ui: &mut egui::Ui) {
        crate::design::brand(ui, "PROJECT", 0.9);
        if let Some(doc) = &self.doc {
            ui.label(RichText::new(doc.title()).strong());
            ui.label(
                RichText::new(
                    doc.path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .unwrap_or_default()
                        .to_string_lossy(),
                )
                .small()
                .weak(),
            );
        }
        ui.add_space(14.);
        ui.separator();
        ui.add_space(8.);
        crate::design::brand(ui, "COMPONENTS", 0.9);
        ui.add(
            egui::TextEdit::singleline(&mut self.palette_search)
                .hint_text("Filter placed components…"),
        );
        ui.label(
            RichText::new("Click a component to place another copy")
                .small()
                .weak(),
        );
        let nodes: Vec<_> = self
            .doc
            .as_ref()
            .map(|d| d.root.children("symbol").cloned().collect())
            .unwrap_or_default();
        egui::ScrollArea::vertical()
            .max_height(180.)
            .show(ui, |ui| {
                for n in nodes {
                    let label = format!("{}  {}", property(&n, "Reference"), property(&n, "Value"));
                    if !label
                        .to_lowercase()
                        .contains(&self.palette_search.to_lowercase())
                    {
                        continue;
                    }
                    if ui
                        .add_sized([ui.available_width(), 30.], egui::Button::new(&label))
                        .clicked()
                    {
                        let id = n
                            .child("lib_id")
                            .map(|n| n.value(1))
                            .unwrap_or("")
                            .to_owned();
                        self.chosen_symbol =
                            self.doc.as_ref().unwrap().instance_definition(&n).cloned();
                        self.chosen_id = Some(id);
                        self.footprint_id = property(&n, "Footprint").into();
                        self.unit = n.child("unit").map(|n| n.number(1) as u32).unwrap_or(1);
                        self.placement_rotation = 0;
                        self.tool = Tool::Symbol;
                        self.selected = None;
                    }
                }
            });
        ui.add_space(10.);
        if ui
            .add_sized(
                [ui.available_width(), 36.],
                egui::Button::new("+  Choose symbol…  A"),
            )
            .clicked()
        {
            self.choose_tool(Tool::Symbol);
        }
        ui.checkbox(&mut self.repeat, "Place repeated copies");
        ui.separator();
        self.tool_strip(ui);
    }
    pub(super) fn properties(&mut self, ui: &mut egui::Ui) {
        crate::design::brand(ui, "PROPERTIES", 1.0);
        ui.separator();
        let node = self
            .selected
            .and_then(|i| self.doc.as_ref()?.root.items().get(i))
            .cloned();
        if let (Some(index), Some(n)) = (self.selected, node) {
            ui.label(RichText::new(n.name().replace('_', " ")).color(ACCENT));
            if n.name() == "symbol" {
                ui.label(n.child("lib_id").map(|n| n.value(1)).unwrap_or(""));
                ui.add_space(8.);
                for key in ["Reference", "Value", "Footprint", "Datasheet"] {
                    let mut value = property(&n, key).to_owned();
                    ui.label(key);
                    let response = ui.text_edit_singleline(&mut value);
                    if response.changed() {
                        self.checkpoint();
                        self.doc.as_mut().unwrap().set_property(index, key, &value);
                    }
                }
                let unit = n.child("unit").map(|n| n.number(1)).unwrap_or(1.);
                ui.label(format!("Unit {unit}"));
                if ui.button("Preview symbol / footprint…").clicked() {
                    self.chosen_id = n.child("lib_id").map(|id| id.value(1).to_owned());
                    self.chosen_symbol =
                        self.doc.as_ref().unwrap().instance_definition(&n).cloned();
                    self.unit = unit as u32;
                    self.footprint_id = property(&n, "Footprint").into();
                    self.load_footprint_preview();
                    self.chooser = true;
                }
            } else if matches!(
                n.name(),
                "text" | "label" | "global_label" | "hierarchical_label"
            ) {
                let mut value = n.value(1).to_owned();
                ui.label("Text");
                if ui.text_edit_multiline(&mut value).changed() {
                    self.checkpoint();
                    let doc = self.doc.as_mut().unwrap();
                    doc.root.items_mut()[index].items_mut()[1] = Node::string(value);
                    doc.dirty = true;
                }
            }
            if n.child("at").is_some() {
                let (x, y, _) = position(&n);
                ui.label(format!("X {x:.3} mm\nY {y:.3} mm"));
            }
            ui.add_space(10.);
            ui.horizontal(|ui| {
                if ui.button("Rotate · R").clicked() {
                    self.checkpoint();
                    self.doc.as_mut().unwrap().rotate(index);
                }
                if ui.button("Delete").clicked() {
                    self.checkpoint();
                    self.doc.as_mut().unwrap().remove(index);
                    self.selected = None;
                }
            });
        } else {
            ui.label("Select a component on the canvas to edit its properties.");
        }
        ui.add_space(28.);
        ui.heading("Schematic hierarchy");
        ui.separator();
        if let Some(doc) = &self.doc {
            ui.label(format!("●  {} (root)", doc.title()));
            let sheets: Vec<_> = doc
                .root
                .children("sheet")
                .map(|n| {
                    (
                        property(n, "Sheetname").to_owned(),
                        property(n, "Sheetfile").to_owned(),
                    )
                })
                .collect();
            for (name, file) in sheets {
                if ui
                    .button(format!("↳ {name}"))
                    .on_hover_text("Open child sheet as its own document")
                    .clicked()
                    && self.may_replace()
                    && let Some(path) = self
                        .doc
                        .as_ref()
                        .and_then(|d| d.path.as_ref())
                        .and_then(|p| p.parent())
                        .map(|p| p.join(&file))
                {
                    self.open_path(&path);
                }
            }
        }
        ui.add_space(24.);
        ui.heading("Grid");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("grid")
                .selected_text(format!("{:.3} mm", self.view.grid))
                .show_ui(ui, |ui| {
                    for grid in [0.254, 0.635, 1.27, 2.54] {
                        ui.selectable_value(&mut self.view.grid, grid, format!("{grid} mm"));
                    }
                });
        });
        ui.checkbox(&mut self.view.show_grid, "Visible");
        if let Some(doc) = &self.doc {
            for warning in doc.warnings() {
                ui.add_space(8.);
                ui.label(
                    RichText::new(warning)
                        .small()
                        .color(Color32::from_rgb(237, 185, 91)),
                );
            }
        }
    }
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if crate::design::button(ui, "Home", "fit", false).clicked() && self.may_replace() {
                self.doc = None;
            }
            if crate::design::button(ui, "Save", "save", false)
                .on_hover_text("Save · Ctrl+S")
                .clicked()
            {
                self.save(false);
            }
            if crate::design::button(ui, "Undo", "undo", false).clicked() {
                self.undo();
            }
            if crate::design::button(ui, "Redo", "redo", false).clicked() {
                self.redo();
            }
            ui.separator();
            for (label, icon, tool) in [
                ("Select", "select", Tool::Select),
                ("Wire", "wire", Tool::Wire),
                ("Symbol", "symbol", Tool::Symbol),
            ] {
                if crate::design::button(ui, label, icon, self.tool == tool).clicked() {
                    self.choose_tool(tool);
                }
            }
            ui.separator();
            if crate::design::button(ui, "Zoom in", "plus", false).clicked() {
                self.view.scale = (self.view.scale * 1.2).min(60.);
            }
            if crate::design::button(ui, "Zoom out", "minus", false).clicked() {
                self.view.scale = (self.view.scale / 1.2).max(0.5);
            }
            if crate::design::button(ui, "Fit", "fit", false).clicked() {
                self.fit = true;
            }
            ui.separator();
            if crate::design::button(ui, "ERC", "erc", false).clicked() {
                self.inspect();
            }
            ui.menu_button("Export", |ui| {
                for kind in ["pdf", "svg", "csv"] {
                    if ui.button(format!("{}…", kind.to_uppercase())).clicked() {
                        self.output(kind);
                        ui.close();
                    }
                }
            });
        });
    }
    pub(super) fn tool_strip(&mut self, ui: &mut egui::Ui) {
        let tools = [
            ("Select", "Esc", Tool::Select),
            ("Symbol", "A", Tool::Symbol),
            ("Wire", "W", Tool::Wire),
            ("Junction", "J", Tool::Junction),
            ("Label", "L", Tool::Label),
            ("Text", "T", Tool::Text),
            ("No connect", "Q", Tool::NoConnect),
            ("Move", "M", Tool::Move),
        ];
        let width = (ui.available_width() - 8.) / 2.;
        egui::Grid::new("placement_tools")
            .num_columns(2)
            .show(ui, |ui| {
                for (index, (label, key, tool)) in tools.into_iter().enumerate() {
                    if ui
                        .add_sized(
                            [width, 28.],
                            egui::Button::new(format!("{label}  {key}"))
                                .selected(self.tool == tool),
                        )
                        .clicked()
                    {
                        self.choose_tool(tool);
                    }
                    if index % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        ui.add_space(12.);
        ui.label(
            RichText::new("Wheel: zoom\nMiddle drag: pan\nR: rotate\nDel: delete")
                .small()
                .weak(),
        );
    }
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        self.canvas_rect = rect;
        let scene = Scene::page_document(self.doc.as_ref().unwrap());
        if self.fit {
            self.view.fit(&scene, rect);
            self.fit = false;
        }
        let active = !self.chooser
            && !self.text_dialog
            && !self.library_dialog
            && !self.show_checks
            && !self.about
            && self.error.is_none();
        if response.hovered()
            && active
            && let Some(pos) = response.hover_pos()
        {
            self.cursor = self.view.snap(self.view.world(pos, rect));
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.01 {
                let before = self.view.world(pos, rect);
                self.view.scale = (self.view.scale * (scroll * 0.002).exp()).clamp(0.5, 60.);
                let after = self.view.world(pos, rect);
                self.view.center.0 += before.0 - after.0;
                self.view.center.1 += before.1 - after.1;
            }
        }
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            let delta = ui.input(|i| i.pointer.delta());
            self.view.center.0 -= delta.x as f64 / self.view.scale as f64;
            self.view.center.1 -= delta.y as f64 / self.view.scale as f64;
        }
        if active
            && self.tool == Tool::Select
            && response.drag_started_by(egui::PointerButton::Primary)
        {
            let origin = ui.input(|i| i.pointer.press_origin());
            if let Some(pos) = origin {
                let p = self.view.world(pos, rect);
                self.selected = canvas::hit(&scene, &self.view, p)
                    .filter(|i| editable(self.doc.as_ref().unwrap().root.items()[*i].name()));
                if self.selected.is_some() {
                    self.checkpoint();
                    self.drag_previous = Some(self.view.snap(p));
                }
            }
        }
        if active
            && self.tool == Tool::Select
            && response.dragged_by(egui::PointerButton::Primary)
            && let (Some(index), Some(previous), Some(pos)) = (
                self.selected,
                self.drag_previous,
                response.interact_pointer_pos(),
            )
        {
            let next = self.view.snap(self.view.world(pos, rect));
            self.doc
                .as_mut()
                .unwrap()
                .translate(index, next.0 - previous.0, next.1 - previous.1);
            self.drag_previous = Some(next);
        }
        if response.drag_stopped() {
            self.drag_previous = None;
        }
        if active
            && response.clicked_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            let raw = self.view.world(pos, rect);
            let p = self.view.snap(raw);
            match self.tool {
                Tool::Select => {
                    self.selected = canvas::hit(&scene, &self.view, raw)
                        .filter(|i| editable(self.doc.as_ref().unwrap().root.items()[*i].name()))
                }
                Tool::Symbol => {
                    if let (Some(id), Some(def)) =
                        (self.chosen_id.clone(), self.chosen_symbol.clone())
                    {
                        self.checkpoint();
                        self.selected = Some(
                            self.doc
                                .as_mut()
                                .unwrap()
                                .add_symbol(&id, &def, p.0, p.1, self.unit),
                        );
                        let index = self.selected.unwrap();
                        for _ in 0..self.placement_rotation {
                            self.doc.as_mut().unwrap().rotate(index);
                        }
                        if !self.footprint_id.is_empty() {
                            self.doc.as_mut().unwrap().set_property(
                                index,
                                "Footprint",
                                &self.footprint_id,
                            );
                        }
                        self.status = format!("Placed {id} · R rotates · Esc cancels");
                        if !self.repeat {
                            self.tool = Tool::Select;
                        }
                    }
                }
                Tool::Wire => {
                    if let Some(a) = self.wire_start {
                        self.checkpoint();
                        let corner = (p.0, a.1);
                        let doc = self.doc.as_mut().unwrap();
                        doc.add_wire(a, corner);
                        doc.add_wire(corner, p);
                        self.wire_start = Some(p);
                        if response.double_clicked() {
                            self.wire_start = None;
                            self.tool = Tool::Select;
                        }
                    } else {
                        self.wire_start = Some(p);
                    }
                }
                Tool::Junction | Tool::NoConnect => {
                    self.checkpoint();
                    self.doc.as_mut().unwrap().add_point(
                        if self.tool == Tool::Junction {
                            "junction"
                        } else {
                            "no_connect"
                        },
                        p.0,
                        p.1,
                    );
                }
                Tool::Text | Tool::Label => {
                    if !self.text.is_empty() {
                        self.checkpoint();
                        self.selected = Some(self.doc.as_mut().unwrap().add_text(
                            if self.tool == Tool::Text {
                                "text"
                            } else {
                                "label"
                            },
                            &self.text,
                            p.0,
                            p.1,
                        ));
                        self.tool = Tool::Select;
                    }
                }
                Tool::Move => {
                    if let Some(index) = self.selected {
                        let n = &self.doc.as_ref().unwrap().root.items()[index];
                        if n.child("at").is_some() {
                            let (x, y, _) = position(n);
                            self.checkpoint();
                            self.doc
                                .as_mut()
                                .unwrap()
                                .translate(index, p.0 - x, p.1 - y);
                            self.tool = Tool::Select;
                        }
                    } else {
                        self.selected = canvas::hit(&scene, &self.view, raw);
                    }
                }
            }
        }
        let painter = ui.painter_at(rect);
        let scene = Scene::page_document(self.doc.as_ref().unwrap());
        canvas::paint(&painter, &self.view, rect, &scene, self.selected, false);
        if response.hovered() {
            let pos = self.view.screen(self.cursor, rect);
            painter.line_segment(
                [pos - Vec2::new(8., 0.), pos + Vec2::new(8., 0.)],
                Stroke::new(1.0_f32, Color32::from_gray(110)),
            );
            painter.line_segment(
                [pos - Vec2::new(0., 8.), pos + Vec2::new(0., 8.)],
                Stroke::new(1.0_f32, Color32::from_gray(110)),
            );
        }
        if active && response.clicked_by(egui::PointerButton::Secondary) {
            self.tool = Tool::Select;
            self.wire_start = None;
        }
        if let Some(a) = self.wire_start {
            let b = self.cursor;
            let corner = (b.0, a.1);
            painter.add(egui::Shape::line(
                vec![
                    self.view.screen(a, rect),
                    self.view.screen(corner, rect),
                    self.view.screen(b, rect),
                ],
                Stroke::new(1.5_f32, Color32::from_rgb(0, 160, 0)),
            ));
        }
        if self.tool == Tool::Symbol
            && active
            && response.hovered()
            && let Some(def) = &self.chosen_symbol
        {
            let mut preview = Document::new();
            let index = preview.add_symbol(
                "preview:Symbol",
                def,
                self.cursor.0,
                self.cursor.1,
                self.unit,
            );
            for _ in 0..self.placement_rotation {
                preview.rotate(index);
            }
            let scene = Scene::from_document(&preview);
            paint_overlay(&painter, &self.view, rect, &scene);
        }
    }
}
