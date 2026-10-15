use super::*;
impl App {
    pub(super) fn pick_symbol(&mut self, id: &str) {
        let result = self.libraries.as_mut().map(|l| l.symbol(id));
        if let Some(result) = result {
            match result {
                Ok(def) => {
                    self.footprint_id = property(&def, "Footprint").into();
                    self.footprint_scene = None;
                    if !self.footprint_id.is_empty() {
                        self.load_footprint_preview();
                    }
                    self.chosen_id = Some(id.into());
                    self.chosen_symbol = Some(def);
                    self.unit = 1;
                    self.placement_rotation = 0;
                }
                Err(e) => self.fail(e),
            }
        }
    }
    pub(super) fn load_footprint_preview(&mut self) {
        self.footprint_scene = None;
        if let Some(libs) = &self.libraries {
            match libs.footprint(&self.footprint_id) {
                Ok(root) => self.footprint_scene = Some(Scene::footprint_preview(&root)),
                Err(e) => self.status = e.to_string(),
            }
        }
    }
    fn confirm_symbol(&mut self) {
        if let Some(id) = &self.chosen_id {
            self.recent_symbols.retain(|s| s != id);
            self.recent_symbols.insert(0, id.clone());
            self.recent_symbols.truncate(20);
        }
        self.chooser = false;
        self.tool = Tool::Symbol;
        self.selected = None;
        self.status = "Click the canvas to place · R rotates · Esc or right click cancels".into();
    }
    pub(super) fn symbol_chooser(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let count = self
            .libraries
            .as_ref()
            .map(|l| l.entries.len())
            .unwrap_or(0);
        egui::Window::new(format!("Choose symbol · {count} available")).id(egui::Id::new("symbol_chooser"))
            .open(&mut open).collapsible(false).default_size([1040.,(ctx.content_rect().height()-160.).max(400.)]).max_size(ctx.content_rect().size()-Vec2::new(60.,90.)).min_size([700.,400.]).show(ctx,|ui| {
                ui.horizontal(|ui| {
                    ui.label("Search");
                    let response=ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Name, description, keywords… e.g. resistor").desired_width(360.));
                    if self.chooser_focus {response.request_focus();self.chooser_focus=false;}
                    if ui.small_button("Clear").clicked(){self.search.clear();}
                    let libs:std::collections::BTreeSet<_>=self.libraries.as_ref().map(|l|l.entries.iter().filter_map(|e|e.id.split_once(':').map(|(l,_)|l.to_owned())).collect()).unwrap_or_default();
                    egui::ComboBox::from_id_salt("library_filter").selected_text(if self.library_filter.is_empty(){"All libraries"}else{&self.library_filter}).width(180.).show_ui(ui,|ui| {
                        ui.selectable_value(&mut self.library_filter,String::new(),"All libraries");
                        for lib in libs {ui.selectable_value(&mut self.library_filter,lib.clone(),lib);}
                    });
                });
                ui.separator();
                let results:Vec<_>=self.libraries.as_ref().map(|l|l.search(&self.search,&self.library_filter).into_iter().map(|e|(e.id.clone(),e.description.clone(),e.source.clone())).collect()).unwrap_or_default();
                let mut rows:Vec<(String,String,bool)>=vec![];
                let browse=self.search.trim().is_empty()&&self.library_filter.is_empty();
                if browse {
                    let mut previous=String::new();
                    for (id,description,_) in &results {let lib=id.split_once(':').map(|(l,_)|l).unwrap_or("");
                        if previous!=lib {rows.push((lib.into(),String::new(),true));previous=lib.into();}
                        if self.expanded_libraries.contains(lib){rows.push((id.clone(),description.clone(),false));}
                    }
                }else{rows=results.iter().map(|(id,desc,_)|(id.clone(),desc.clone(),false)).collect();}
                let height=(ctx.content_rect().height()-280.).clamp(260.,440.);
                ui.columns(2,|cols| {
                    cols[0].horizontal(|ui| {ui.strong("Library : Symbol");ui.separator();ui.strong("Description");});
                    egui::ScrollArea::vertical().id_salt(("symbol_results",&self.search,&self.library_filter)).max_height(height-130.).show_rows(&mut cols[0],28.,rows.len(),|ui,range| {
                        for index in range {let (id,desc,group)=&rows[index];
                            if *group {let expanded=self.expanded_libraries.contains(id);if ui.add_sized([ui.available_width(),28.],egui::Button::new(format!("{}  {id}",if expanded{"−"}else{"+"})).truncate()).clicked(){if expanded{self.expanded_libraries.remove(id);}else{self.expanded_libraries.insert(id.clone());}}continue;}
                            let response=ui.add_sized([ui.available_width(),28.],egui::Button::selectable(self.chosen_id.as_ref()==Some(id),format!("{id}   ·   {desc}")).truncate()).on_hover_text(format!("{id}\n{desc}"));
                            if response.clicked(){self.pick_symbol(id);}
                            if response.double_clicked() && self.doc.is_some(){self.pick_symbol(id);self.confirm_symbol();}
                        }
                    });
                    cols[0].separator();
                    if let Some(def)=&self.chosen_symbol {
                        cols[0].strong(self.chosen_id.as_deref().unwrap_or(""));cols[0].label(property(def,"Description"));
                        cols[0].label(RichText::new(format!("Keywords: {}",property(def,"ki_keywords"))).small().weak());
                        cols[0].label(format!("Reference: {}?",property(def,"Reference").trim_end_matches('?')));
                        let sheet=property(def,"Datasheet");if sheet.starts_with("https://") || sheet.starts_with("http://"){cols[0].hyperlink_to("Open datasheet",sheet);}
                    }
                    cols[1].label(RichText::new("SYMBOL PREVIEW").monospace().strong());
                    if let Some(def)=&self.chosen_symbol {
                        let scene=Scene::symbol_preview_unit(def,self.unit);preview(&mut cols[1],&scene,(height*0.40).min(230.));
                        let max=def.children("symbol").filter_map(|s|s.value(1).rsplit('_').nth(1)?.parse::<u32>().ok()).max().unwrap_or(1).max(1);
                        cols[1].horizontal(|ui|{ui.label("Unit");ui.add(egui::DragValue::new(&mut self.unit).range(1..=max));});
                    }else {cols[1].label("Select a symbol to preview its actual KiCad geometry.");cols[1].add_space(150.);}
                    cols[1].separator();cols[1].label(RichText::new("FOOTPRINT PREVIEW · 2D").monospace().strong());
                    cols[1].horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.footprint_search).hint_text("Filter footprints…").desired_width(160.));
                        let needle=self.footprint_search.to_lowercase();
                        egui::ComboBox::from_id_salt("footprint_choice").selected_text(if self.footprint_id.is_empty(){"No footprint specified"}else{&self.footprint_id}).width(220.).show_ui(ui,|ui| {
                            if ui.selectable_label(self.footprint_id.is_empty(),"No footprint specified").clicked(){self.footprint_id.clear();self.footprint_scene=None;}
                            let found:Vec<_>=self.footprint_ids.iter().filter(|s|s.to_lowercase().contains(&needle)).cloned().collect();
                            egui::ScrollArea::vertical().max_height(240.).show_rows(ui,24.,found.len(),|ui,range|{for i in range {let id=&found[i];if ui.selectable_label(&self.footprint_id==id,id).clicked(){self.footprint_id=id.clone();self.load_footprint_preview();}}});
                        });
                    });
                    if let Some(scene)=&self.footprint_scene {preview(&mut cols[1],scene,(height*0.35).min(200.));}
                    else {cols[1].label(if self.footprint_id.is_empty(){"No footprint specified. Choose a footprint above to preview and assign it."}else{"Footprint unavailable in configured libraries. Choose an available footprint above."});}
                });
                ui.separator();ui.horizontal(|ui| {
                    ui.label(format!("{} matches · searching every loaded library",results.len()));
                    ui.checkbox(&mut self.repeat,"Repeated copies");
                    if ui.add_enabled(self.chosen_symbol.is_some()&&self.doc.is_some(),egui::Button::new(RichText::new("Select").color(Color32::WHITE)).fill(ACCENT)).clicked(){self.confirm_symbol();}
                    if ui.button("Cancel").clicked(){self.chooser=false;self.tool=Tool::Select;}
                });
            });
        if ctx.input(|i| i.key_pressed(Key::Enter))
            && self.chosen_symbol.is_some()
            && self.doc.is_some()
        {
            self.confirm_symbol();
        }
        if !open {
            self.chooser = false;
            self.tool = Tool::Select;
        }
    }
}
fn preview(ui: &mut egui::Ui, scene: &Scene, height: f32) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let (a, b) = scene.content_bounds();
    let mut view = View {
        show_grid: false,
        ..View::default()
    };
    view.center = ((a.0 + b.0) / 2., (a.1 + b.1) / 2.);
    view.scale = (rect.width() / ((b.0 - a.0 + 3.) as f32))
        .min(rect.height() / ((b.1 - a.1 + 3.) as f32))
        .clamp(0.2, 35.);
    canvas::paint(&ui.painter_at(rect), &view, rect, scene, None, false);
}
