use super::*;
impl App {
    pub(super) fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            crate::design::brand(ui, "JENGA", 1.2);
            ui.separator();
            ui.menu_button("File", |ui| {
                if ui.button("Import asset into this project...").clicked(){self.asset_import_dialog(false);ui.close();}
                if ui.button("Import asset into personal library...").clicked(){self.asset_import_dialog(true);ui.close();}
                if ui.button("Import KiCad project copy...").clicked(){self.transfer_dialog("import_project");ui.close();}
                if ui.button("Import schematic / PCB into project...").clicked(){self.transfer_dialog("import_document");ui.close();}
                if ui.button("Move project to folder...").clicked(){self.transfer_dialog("move_project");ui.close();}
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
                if ui.button("Save project copy to...").clicked(){self.copy_project_to();ui.close();}
                if ui.button("Delete project… (recoverable)").clicked() {
                    self.delete_current_project();
                    ui.close();
                }
                if ui.button("Rename schematic / project…").clicked() {
                    if self.doc.is_some() && self.save(false) {
                        let doc = self.doc.as_ref().unwrap();
                        self.rename_name = doc.project_name();
                        self.rename_project = doc
                            .path
                            .as_ref()
                            .is_some_and(|p| p.with_extension("kicad_pro").exists());
                        self.rename_folder = false;
                        self.rename_dialog = true;
                    }
                    ui.close();
                }
                if ui.button("Page settings / title block…").clicked() {
                    self.page_dialog = true;
                    ui.close();
                }
                ui.separator();
                for kind in ["svg", "pdf", "xlsx", "csv", "kicad_sch"] {
                    if ui
                        .button(format!("Export {}…", export_label(kind)))
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
                if ui.button("Arrange unwired selected symbols").clicked(){
                    if let Some(doc)=&self.doc{
                        let ids:Vec<_>=doc.root.items().iter().enumerate().filter(|(i,n)|n.name()=="symbol"&&(self.copy_all||self.selected==Some(*i)||n.child("uuid").is_some_and(|id|self.copy_selection.iter().any(|s|s==id.value(1))))).filter_map(|(_,n)|n.child("uuid").map(|id|id.value(1).to_owned())).collect();
                        if !ids.is_empty(){if let Err(e)=self.control_request("edit",&serde_json::json!({"actions":[{"op":"arrange_symbols","uuids":ids,"at":[20.32,25.4]}],"actor":"user"})){self.fail(e);}}
                    }
                    ui.close();
                }
                if ui.button("Select all objects   Ctrl+A").clicked() {
                    self.copy_all = true;
                    ui.close();
                }
                if ui.button("Copy   Ctrl+C").clicked() {
                    self.copy_clipboard(ui.ctx());
                    ui.close();
                }
                if ui.button("Cut   Ctrl+X").clicked() {
                    self.cut_clipboard(ui.ctx());
                    ui.close();
                }
                if ui.button("Paste at cursor   Ctrl+V").clicked() {
                    self.paste_clipboard();
                    ui.close();
                }
                ui.separator();
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
                if ui.button("Annotate references…").clicked() {
                    if let Err(e) = self.control_request(
                        "edit",
                        &serde_json::json!({"actions":[{"op":"annotate"}],"actor":"user"}),
                    ) {
                        self.fail(e);
                    }
                    ui.close();
                }
                if ui.button("Change history…").clicked() {
                    self.observe_history();
                    self.flush_human_history();
                    self.history_dialog = true;
                    ui.close();
                }
                if ui.button("Electrical rules check").clicked() {
                    self.inspect();
                    ui.close();
                }
            });
            ui.menu_button("Preferences", |ui| {
                if ui.button("Symbol editor…").clicked() {
                    self.symbol_editor = Some(Default::default());
                    ui.close();
                }
                if ui.button("Library sources…").clicked() {
                    self.library_dialog = true;
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Features & help…").clicked() {
                    self.help_dialog = true;
                    ui.close();
                }
                if ui.button("Connect Assistant…").clicked() {
                    self.connect_dialog = true;
                    ui.close();
                }
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
    pub(super) fn palette(&mut self, ui: &mut egui::Ui) {
        crate::design::brand(ui, "PROJECT", 0.65);
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
        ui.add_space(4.);
        ui.separator();
        ui.add_space(8.);
        crate::design::brand(ui, "COMPONENTS", 0.65);
        ui.add(
            egui::TextEdit::singleline(&mut self.palette_search)
                .hint_text("Filter components…")
                .desired_width(ui.available_width()),
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
            .id_salt("component_list_only")
            .max_height(ui.available_height().max(0.))
            .auto_shrink([false, false])
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
                        .add_sized(
                            [ui.available_width(), 22.],
                            egui::Button::new(RichText::new(&label).monospace().size(10.5))
                                .truncate(),
                        )
                        .on_hover_text(&label)
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
    }
    pub(super) fn palette_footer(&mut self, ui: &mut egui::Ui) {
        if ui.small_button("Export BOM · Excel…").clicked() {
            self.output("xlsx");
        }
        ui.separator();
        if ui
            .add_sized(
                [ui.available_width(), 26.],
                egui::Button::new("+  Symbol…  A"),
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
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut value).desired_width(ui.available_width()),
                    );
                    if response.changed() {
                        self.checkpoint();
                        self.doc.as_mut().unwrap().set_property(index, key, &value);
                    }
                }
                let unit = n.child("unit").map(|n| n.number(1)).unwrap_or(1.);
                egui::CollapsingHeader::new("Reference / value appearance").show(ui, |ui| {
                    for key in ["Reference", "Value"] {
                        if let Some(field) = n.children("property").find(|v| v.value(1) == key) {
                            let mut size = jenga_core::scene::font_size(field);
                            ui.horizontal(|ui| {
                                ui.label(key);
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut size)
                                            .range(0.3..=20.)
                                            .speed(0.1)
                                            .suffix(" mm"),
                                    )
                                    .changed()
                                {
                                    self.checkpoint();
                                    if let Err(e) = jenga_tools::formatting::apply(
                                        self.doc.as_mut().unwrap().root.items_mut()[index]
                                            .items_mut()
                                            .iter_mut()
                                            .find(|v| v.name() == "property" && v.value(1) == key)
                                            .unwrap(),
                                        &serde_json::json!({"size_mm":size}),
                                    ) {
                                        self.fail(e);
                                    }
                                    self.doc.as_mut().unwrap().dirty = true;
                                }
                            });
                        }
                    }
                    if ui.button("Keep reference and value upright").clicked() {
                        self.checkpoint();
                        for field in self.doc.as_mut().unwrap().root.items_mut()[index]
                            .items_mut()
                            .iter_mut()
                            .filter(|v| {
                                v.name() == "property"
                                    && ["Reference", "Value"].contains(&v.value(1))
                            })
                        {
                            let _ = jenga_tools::formatting::apply(
                                field,
                                &serde_json::json!({"upright":true}),
                            );
                        }
                        self.doc.as_mut().unwrap().dirty = true;
                    }
                });
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
                let mut size = jenga_core::scene::font_size(&n);
                ui.horizontal(|ui| {
                    ui.label("Font size");
                    if ui
                        .add(
                            egui::DragValue::new(&mut size)
                                .range(0.3..=20.)
                                .speed(0.1)
                                .suffix(" mm"),
                        )
                        .changed()
                    {
                        self.checkpoint();
                        let _ = jenga_tools::formatting::apply(
                            &mut self.doc.as_mut().unwrap().root.items_mut()[index],
                            &serde_json::json!({"size_mm":size}),
                        );
                        self.doc.as_mut().unwrap().dirty = true;
                    }
                });
                ui.horizontal(|ui| {
                    for align in ["left", "center", "right"] {
                        if ui.button(align).clicked() {
                            self.checkpoint();
                            let _ = jenga_tools::formatting::apply(
                                &mut self.doc.as_mut().unwrap().root.items_mut()[index],
                                &serde_json::json!({"align":align}),
                            );
                            self.doc.as_mut().unwrap().dirty = true;
                        }
                    }
                });
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
        ui.add_space(16.);
        crate::design::brand(ui, "PAGES", 0.7);
        if ui.button("Switch / manage pages…").clicked() {
            self.workspace_ui.manager = true;
        }
        ui.separator();
        if let Some(doc) = &self.doc {
            let child = doc
                .root
                .child("sheet_instances")
                .is_some_and(|s| s.children("path").any(|p| p.value(1) != "/"));
            ui.label(format!(
                "●  {} ({})",
                doc.title(),
                if child { "page" } else { "root schematic" }
            ));
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
        ui.add_space(16.);
        crate::design::brand(ui, "GRID", 0.7);
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
        if ui.button("Page / title block…").clicked() {
            self.page_dialog = true;
        }
        if let Some(issue) = &self.active_issue {
            ui.separator();
            ui.label(RichText::new(&issue.message).color(ACCENT));
            ui.label(&issue.suggestion);
            if ui.small_button("Clear issue marker").clicked() {
                self.active_issue = None;
            }
        }
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
        ui.horizontal_wrapped(|ui| {
            if crate::design::button(ui, "Project", "project", false).clicked() {
                self.workspace_ui.root = self.doc.as_ref().and_then(|d| d.path.clone());
                self.workspace_ui.show = true;
            }
            if crate::design::button(ui, "Home", "home", false).clicked() && self.may_replace() {
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
            if crate::design::button(ui, "Save as", "save_as", false)
                .on_hover_text("Save schematic as… · Ctrl+Shift+S")
                .clicked()
            {
                self.save(true);
            }
            if crate::design::button(ui, "File location", "folder", false).clicked() {
                self.open_file_location(self.doc.as_ref().and_then(|d| d.path.clone()));
            }
            if crate::design::button(ui, "Delete", "delete", false)
                .on_hover_text("Delete project • export first / recoverable")
                .clicked()
            {
                self.delete_current_project();
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
            if crate::design::button(ui, "Add page", "symbol", false)
                .on_hover_text("Add a native child page to this schematic")
                .clicked()
            {
                self.workspace_ui.page_dialog = true;
                self.workspace_ui.hierarchical = true;
                self.workspace_ui.page_name = "NewPage".into();
            }
            ui.menu_button("Pages", |ui| {
                if ui.button("Switch / rename / organise…").clicked() {
                    self.workspace_ui.manager = true;
                    ui.close();
                }
                if ui.button("Title block / page settings…").clicked() {
                    self.page_dialog = true;
                    ui.close();
                }
                ui.separator();
                if let Some(path) = self.doc.as_ref().and_then(|d| d.path.as_ref())
                    && let Ok(project) = jenga_tools::project_workspace::discover(path)
                {
                    for page in project.pages {
                        if ui
                            .button(&page.name)
                            .on_hover_text(page.file.display().to_string())
                            .clicked()
                        {
                            self.open_path(&page.file);
                            ui.close();
                        }
                    }
                }
            });
            ui.menu_button("Export", |ui| {
                if ui.button("Selected / all project pages…").clicked() {
                    self.workspace_ui.export_dialog = true;
                    self.workspace_ui.selected_pages = self
                        .doc
                        .as_ref()
                        .and_then(|d| d.path.clone())
                        .into_iter()
                        .collect();
                    ui.close();
                }
                for kind in ["pdf", "svg", "xlsx", "csv", "kicad_sch"] {
                    if ui.button(format!("{}…", export_label(kind))).clicked() {
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
            ("No conn.", "Q", Tool::NoConnect),
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
                            egui::Button::new(RichText::new(format!("{label}  {key}")).size(10.))
                                .truncate()
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
        ui.add_space(4.);
        ui.label(
            RichText::new("Wheel: zoom · Drag: pan\nR: rotate · Del: delete")
                .small()
                .weak(),
        );
    }
    pub(super) fn canvas_click(&mut self, raw: Point, double_clicked: bool) {
        self.copy_all = false;
        let mut p = self.view.snap(raw);
        let scene = Scene::from_document(self.doc.as_ref().unwrap());
        if matches!(
            self.tool,
            Tool::Wire | Tool::NoConnect | Tool::Junction | Tool::Label
        ) {
            let tolerance = (8. / self.view.scale as f64).min(2.54);
            if let Some(pin) = scene
                .pins
                .iter()
                .filter(|pin| (pin.at.0 - raw.0).hypot(pin.at.1 - raw.1) <= tolerance)
                .min_by(|a, b| {
                    ((a.at.0 - raw.0).hypot(a.at.1 - raw.1))
                        .total_cmp(&((b.at.0 - raw.0).hypot(b.at.1 - raw.1)))
                })
            {
                p = pin.at;
            }
        }
        match self.tool {
            Tool::Select => {
                let previous = self.selected;
                self.selected = canvas::hit(&scene, &self.view, raw)
                    .filter(|i| editable(self.doc.as_ref().unwrap().root.items()[*i].name()));
                let uuid = |i: usize| {
                    self.doc.as_ref().unwrap().root.items()[i]
                        .child("uuid")
                        .map(|n| n.value(1).to_owned())
                };
                if self.selection_add {
                    if self.copy_selection.is_empty()
                        && let Some(id) = previous.and_then(uuid)
                    {
                        self.copy_selection.push(id);
                    }
                    if let Some(id) = self.selected.and_then(uuid) {
                        if self.copy_selection.contains(&id) {
                            self.copy_selection.retain(|s| s != &id);
                        } else {
                            self.copy_selection.push(id);
                        }
                    }
                    self.status = format!(
                        "{} objects selected · Ctrl+C to copy",
                        self.copy_selection.len()
                    );
                } else {
                    self.copy_selection.clear();
                }
                self.selection_add = false;
            }
            Tool::Symbol => {
                if let (Some(id), Some(def)) = (self.chosen_id.clone(), self.chosen_symbol.clone())
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
                    if double_clicked {
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
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        self.canvas_rect = rect;
        let scene = Scene::page_document(self.doc.as_ref().unwrap());
        if self.fit {
            self.view.fit(&scene, rect);
            self.fit = false;
        }
        let active = !self.connect_dialog
            && !self.help_dialog
            && !self.chooser
            && !self.text_dialog
            && !self.library_dialog
            && !self.show_checks
            && !self.page_dialog
            && !self.history_dialog
            && !self.rename_dialog
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
            self.selection_add = ui.input(|i| i.modifiers.shift);
            self.canvas_click(raw, response.double_clicked());
        }

        let painter = ui.painter_at(rect);
        let scene = Scene::page_document(self.doc.as_ref().unwrap());
        canvas::paint(&painter, &self.view, rect, &scene, self.selected, false);
        if self.copy_all || !self.copy_selection.is_empty() {
            for (i, n) in self
                .doc
                .as_ref()
                .unwrap()
                .root
                .items()
                .iter()
                .enumerate()
                .filter(|(_, n)| editable(n.name()))
            {
                if !self.copy_all
                    && !n
                        .child("uuid")
                        .is_some_and(|n| self.copy_selection.iter().any(|id| id == n.value(1)))
                {
                    continue;
                }
                if let Some((a, b)) = scene.object_bounds(i) {
                    painter.rect_stroke(
                        egui::Rect::from_two_pos(
                            self.view.screen(a, rect),
                            self.view.screen(b, rect),
                        )
                        .expand(3.),
                        0.,
                        Stroke::new(1_f32, ACCENT),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
        if let Some(issue) = &self.active_issue
            && let Some(at) = issue.location
        {
            let p = self.view.screen(at, rect);
            let pulse = ui.input(|i| i.time) as f32;
            painter.circle_stroke(p, 10. + (pulse * 3.).sin() * 2., Stroke::new(2_f32, ACCENT));
            painter.line_segment(
                [p - Vec2::new(15., 0.), p + Vec2::new(15., 0.)],
                Stroke::new(1_f32, ACCENT),
            );
            painter.line_segment(
                [p - Vec2::new(0., 15.), p + Vec2::new(0., 15.)],
                Stroke::new(1_f32, ACCENT),
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(80));
        }
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

fn export_label(kind: &str) -> &str {
    match kind {
        "xlsx" => "BOM · Excel (.xlsx)",
        "csv" => "BOM · CSV",
        "kicad_sch" => "Native schematic (.kicad_sch)",
        "pdf" => "PDF",
        "svg" => "SVG",
        _ => kind,
    }
}
