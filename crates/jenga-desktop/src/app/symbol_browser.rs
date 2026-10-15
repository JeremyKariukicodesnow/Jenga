use super::*;
use serde_json::json;
pub(super) struct SymbolBrowser {
    pub open: bool,
    pub open_epoch: u64,
    was_open: bool,
    pub project_only: bool,
    query: String,
    manage: Option<(PathBuf, String, String)>,
    rename: String,
    checked: u128,
    signature: String,
    selected: Option<(String, PathBuf)>,
}
impl Default for SymbolBrowser {
    fn default() -> Self {
        Self {
            open: false,
            open_epoch: 0,
            was_open: false,
            project_only: true,
            query: String::new(),
            manage: None,
            rename: String::new(),
            checked: 0,
            signature: String::new(),
            selected: None,
        }
    }
}
impl App {
    fn custom_sources(&self) -> Vec<(String, PathBuf)> {
        let mut folders = std::collections::BTreeMap::new();
        if let Ok(p) = jenga_tools::symbol_editor::personal_folder() {
            folders.insert(p, "Global personal symbols".to_owned());
        }
        for path in self
            .settings
            .recent
            .iter()
            .chain(self.doc.as_ref().and_then(|d| d.path.as_ref()))
        {
            if let Some(folder) = path.parent() {
                folders.insert(
                    folder.to_owned(),
                    format!(
                        "Project · {}",
                        folder.file_name().unwrap_or_default().to_string_lossy()
                    ),
                );
            }
        }
        let mut result = vec![];
        for (folder, label) in folders {
            if let Ok(files) = std::fs::read_dir(folder.join("libraries")) {
                for file in files.flatten() {
                    if file.path().extension().is_some_and(|e| e == "kicad_sym") {
                        result.push((label.clone(), file.path()));
                    }
                }
            }
        }
        result
    }
    fn symbol_cards(
        &mut self,
        ui: &mut egui::Ui,
        entries: &[(String, PathBuf)],
        personal: bool,
        custom: bool,
    ) {
        let columns = ((ui.available_width() + 10.) / 210.).floor().max(1.) as usize;
        let width = ((ui.available_width() - 10. * columns.saturating_sub(1) as f32)
            / columns as f32)
            .max(120.);
        for row in entries.chunks(columns) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.;
                for (name, path) in row {
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(width, 190.), egui::Sense::click());
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, 6., Color32::from_rgb(253, 252, 248));
                    painter.rect_stroke(
                        rect,
                        6.,
                        Stroke::new(
                            1_f32,
                            if response.hovered() {
                                ACCENT
                            } else {
                                Color32::from_gray(205)
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    let mut title_job = egui::text::LayoutJob::simple(
                        name.clone(),
                        egui::FontId::monospace(13.),
                        Color32::from_gray(22),
                        width - 18.,
                    );
                    title_job.wrap.max_rows = 2;
                    title_job.wrap.break_anywhere = true;
                    title_job.wrap.overflow_character = Some('…');
                    let title = painter.layout_job(title_job);
                    painter.galley(rect.min + Vec2::new(9., 8.), title, Color32::from_gray(22));
                    if ui.is_rect_visible(rect) {
                        if let Ok(def) = Libraries::symbol_from_file(path, name) {
                            let scene = Scene::symbol_preview_unit(&def, 1);
                            let area = egui::Rect::from_min_size(
                                rect.min + Vec2::new(9., 42.),
                                Vec2::new(width - 18., 104.),
                            );
                            let mut view = View {
                                show_grid: false,
                                ..Default::default()
                            };
                            view.fit(&scene, area);
                            canvas::paint(&ui.painter_at(area), &view, area, &scene, None, false);
                        }
                    }
                    let area = egui::Rect::from_min_size(
                        rect.min + Vec2::new(8., 158.),
                        Vec2::new(width - 16., 26.),
                    );
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(area)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    child.set_clip_rect(area);
                    let edit = child
                        .small_button(if custom { "Edit" } else { "Edit copy" })
                        .clicked();
                    let promote =
                        custom && !personal && child.small_button("Make global").clicked();
                    if custom {
                        child.menu_button("⋯", |ui| {
                            for action in ["rename", "export", "delete"] {
                                if ui.button(action).clicked() {
                                    self.symbol_browser.manage =
                                        Some((path.clone(), name.clone(), action.into()));
                                    self.symbol_browser.rename = name.clone();
                                    ui.close();
                                }
                            }
                        });
                    }
                    if response.hovered() {
                        self.symbol_browser.selected = Some((name.clone(), path.clone()));
                    }
                    if edit
                        || response
                            .on_hover_text(format!("{} · click to edit", path.display()))
                            .clicked()
                    {
                        self.open_library_symbol(path, name, personal);
                    }
                    if promote {
                        self.promote_symbol(path, name);
                    }
                }
            });
            ui.add_space(10.);
        }
    }
    pub(super) fn project_symbol_cards(&mut self, ui: &mut egui::Ui, folder: &Path) {
        let mut any = false;
        for (_, path) in self
            .custom_sources()
            .into_iter()
            .filter(|(_, p)| p.starts_with(folder))
        {
            if let Ok(text) = std::fs::read_to_string(&path)
                && let Ok(root) = jenga_core::sexpr::parse(&text)
            {
                let entries: Vec<_> = root
                    .children("symbol")
                    .map(|n| (n.value(1).to_owned(), path.clone()))
                    .collect();
                if entries.is_empty() {
                    continue;
                }
                any = true;
                egui::CollapsingHeader::new(format!(
                    "Project library · {} · {} symbols",
                    path.file_stem().unwrap_or_default().to_string_lossy(),
                    entries.len()
                ))
                .id_salt((&path, self.symbol_browser.open_epoch))
                .default_open(true)
                .show(ui, |ui| self.symbol_cards(ui, &entries, false, true));
            }
        }
        if !any {
            ui.label("No custom project symbols yet. Use Symbol editor to create one.");
        }
    }
    fn open_library_symbol(&mut self, path: &Path, name: &str, personal: bool) {
        let custom = self.custom_sources().iter().any(|(_, p)| p == path);
        if custom && !personal {
            if let Some(folder) = path.parent().and_then(|p| p.parent()) {
                if self
                    .doc
                    .as_ref()
                    .and_then(|d| d.path.as_ref())
                    .and_then(|p| p.parent())
                    != Some(folder)
                {
                    if let Ok(files) = std::fs::read_dir(folder) {
                        if let Some(file) = files
                            .flatten()
                            .map(|f| f.path())
                            .find(|p| p.extension().is_some_and(|e| e == "kicad_sch"))
                        {
                            self.open_path(&file);
                            if self
                                .doc
                                .as_ref()
                                .and_then(|d| d.path.as_ref())
                                .and_then(|p| p.parent())
                                != Some(folder)
                            {
                                return;
                            }
                        } else {
                            self.status =
                                "Open or save this symbol's project before editing its library"
                                    .into();
                            return;
                        }
                    }
                }
            }
        }
        match Libraries::symbol_from_file(path, name) {
            Ok(def) => {
                let mut e = symbols::SymbolEditor::from_definition(
                    def,
                    &format!(
                        "{}:{name}",
                        path.file_stem().unwrap_or_default().to_string_lossy()
                    ),
                    personal,
                );
                if custom {
                    e.set_library(
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .as_ref(),
                    );
                }
                self.symbol_editor = Some(e);
            }
            Err(e) => self.fail(e),
        }
    }
    fn promote_symbol(&mut self, path: &Path, name: &str) {
        let result = (|| -> anyhow::Result<serde_json::Value> {
            let def = Libraries::symbol_from_file(path, name)?;
            let personal = jenga_tools::symbol_editor::personal_folder()?
                .join("libraries/SharedSymbols.kicad_sym");
            let mut p = json!({"name":name,"library":"SharedSymbols","scope":"personal","native_source":def.encoded(),"license":"Original source library license applies; not relicensed","attribution":path.display().to_string(),"actor":"user"});
            if personal.exists() {
                let root = jenga_core::sexpr::parse(&std::fs::read_to_string(&personal)?)?;
                anyhow::ensure!(
                    !root.children("symbol").any(|n| n.value(1) == name),
                    "Global symbol already exists; use Edit to choose a distinct symbol name"
                );
                p["expected_revision"] =
                    json!(jenga_tools::journal::revision_text(&root.encoded()));
            }
            self.control_request("save_symbol", &p)
        })();
        match result {
            Ok(_) => {
                self.reload_libraries();
                self.status = format!("{name} is now available globally in SharedSymbols");
            }
            Err(e) => self.fail(e),
        }
    }
    pub(super) fn symbol_browser_dialog(&mut self, ctx: &egui::Context) {
        let now = jenga_tools::journal::now();
        if now > self.symbol_browser.checked {
            self.symbol_browser.checked = now + 2000;
            let signature = format!(
                "{:?}",
                self.custom_sources()
                    .iter()
                    .map(|(_, p)| (
                        p.clone(),
                        std::fs::metadata(p).and_then(|m| m.modified()).ok()
                    ))
                    .collect::<Vec<_>>()
            );
            if signature != self.symbol_browser.signature {
                let first = self.symbol_browser.signature.is_empty();
                self.symbol_browser.signature = signature;
                if !first {
                    self.reload_libraries();
                }
            }
        }
        self.symbol_manage_dialog(ctx);
        if !self.symbol_browser.open {
            self.symbol_browser.was_open = false;
            return;
        }
        if !self.symbol_browser.was_open {
            self.symbol_browser.open_epoch += 1;
            self.symbol_browser.was_open = true;
        }
        if self.symbol_browser.selected.is_none() {
            for (_, file) in self.custom_sources() {
                if let Ok(text) = std::fs::read_to_string(&file)
                    && let Ok(root) = jenga_core::sexpr::parse(&text)
                {
                    if let Some(n) = root.children("symbol").next() {
                        self.symbol_browser.selected = Some((n.value(1).into(), file));
                        break;
                    }
                }
            }
        }
        let mut open = true;
        egui::Window::new("Symbols · libraries and your projects").open(&mut open).default_size([1060.,680.]).show(ctx,|ui|{
            ui.horizontal_wrapped(|ui|{
                if ui.button("Create global symbol").clicked(){self.symbol_editor=Some(Default::default());}
                if self.doc.is_some()&&ui.button("Create project symbol").clicked(){let mut e=symbols::SymbolEditor::default();e.set_personal(false);self.symbol_editor=Some(e);}
                ui.checkbox(&mut self.symbol_browser.project_only,"Custom libraries only");
            });
            ui.add(egui::TextEdit::singleline(&mut self.symbol_browser.query).hint_text("Search symbols…").desired_width(f32::INFINITY));
            ui.label("Click a library heading to expand its cards. Click a card to edit; hover to preview.");
            let sources=self.custom_sources();let query=self.symbol_browser.query.to_lowercase();
            let list_width=(ui.available_width()*0.66).max(240.);
            ui.horizontal_top(|ui|{
                ui.allocate_ui_with_layout(Vec2::new(list_width,560.),egui::Layout::top_down(egui::Align::Min),|ui|{
                    egui::ScrollArea::vertical().id_salt("symbol_card_list").max_height(560.).show(ui,|ui|{
                        ui.set_width(list_width-16.);
                        for(group,path)in sources{
                            if let Ok(text)=std::fs::read_to_string(&path)&&let Ok(root)=jenga_core::sexpr::parse(&text){
                                let entries:Vec<_>=root.children("symbol").filter(|n|format!("{} {}",n.value(1),property(n,"Description")).to_lowercase().contains(&query)).map(|n|(n.value(1).to_owned(),path.clone())).collect();
                                if entries.is_empty(){continue;}
                                egui::CollapsingHeader::new(RichText::new(format!("{group} · {} · {} symbols",path.file_stem().unwrap_or_default().to_string_lossy(),entries.len())).strong().color(ACCENT)).id_salt((&path,self.symbol_browser.open_epoch)).default_open(false).show(ui,|ui|self.symbol_cards(ui,&entries,group.starts_with("Global"),true));
                            }
                        }
                        if !self.symbol_browser.project_only{
                            if let Some(libs)=&self.libraries{
                                let entries:Vec<_>=libs.search(&self.symbol_browser.query,"").into_iter().take(120).map(|e|(e.id.split_once(':').map(|p|p.1).unwrap_or(&e.id).to_owned(),e.file.clone())).collect();
                                egui::CollapsingHeader::new(RichText::new("Installed libraries · edit a personal copy").strong().color(ACCENT)).id_salt(("installed",self.symbol_browser.open_epoch)).default_open(false).show(ui,|ui|self.symbol_cards(ui,&entries,true,false));
                                ui.label("Search to narrow installed libraries; showing up to 120 matches.");
                            }else{ui.spinner();ui.label("Indexing library symbols…");}
                        }
                    });
                });
                ui.separator();ui.vertical(|ui|{
                    ui.set_min_width(220.);ui.heading("Symbol preview");
                    if let Some((name,path))=self.symbol_browser.selected.clone(){
                        ui.label(RichText::new(&name).strong());ui.label(path.file_stem().unwrap_or_default().to_string_lossy());
                        if let Ok(def)=Libraries::symbol_from_file(&path,&name){let scene=Scene::symbol_preview_unit(&def,1);let(rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width().max(220.),310.),egui::Sense::hover());let mut view=View{show_grid:false,..Default::default()};view.fit(&scene,rect);canvas::paint(&ui.painter_at(rect),&view,rect,&scene,None,false);}
                        if ui.button("Edit selected symbol").clicked(){let personal=jenga_tools::symbol_editor::personal_folder().is_ok_and(|p|path.starts_with(p)) || !self.custom_sources().iter().any(|(_,p)|p==&path);self.open_library_symbol(&path,&name,personal);}
                    }
                });
            });
        });
        self.symbol_browser.open = open;
    }
    fn symbol_manage_dialog(&mut self, ctx: &egui::Context) {
        let Some((path, name, action)) = self.symbol_browser.manage.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new(format!("{action} symbol · {name}")).open(&mut open).collapsible(false).show(ctx,|ui|{
            if action=="rename"{ui.text_edit_singleline(&mut self.symbol_browser.rename);}
            ui.label("Placed schematic instances keep their embedded definition. Library changes are backed up.");
            if ui.button("Confirm").clicked(){let result=(||->anyhow::Result<serde_json::Value>{
                let root=jenga_core::sexpr::parse(&std::fs::read_to_string(&path)?)?;let base=path.parent().and_then(|p|p.parent()).unwrap();
                let personal=jenga_tools::symbol_editor::personal_folder().is_ok_and(|p|p==base);let mut context=Document::new();context.path=Some(base.join("context.kicad_sch"));
                let mut p=json!({"library":path.file_stem().unwrap().to_string_lossy(),"scope":if personal{"personal"}else{"project"},"name":name,"action":action,"new_name":self.symbol_browser.rename,"expected_revision":jenga_tools::journal::revision_text(&root.encoded())});
                if action=="export"{let out=rfd::FileDialog::new().set_file_name(format!("{name}.kicad_sym")).save_file().ok_or_else(||anyhow::anyhow!("Export cancelled"))?;p["path"]=json!(out);}
                jenga_tools::library_manage::manage(Some(&context),&p)
            })();match result{Ok(_)=>{self.symbol_browser.manage=None;self.reload_libraries();self.status=format!("Symbol {action} complete");},Err(e)=>self.fail(e)}}
            if ui.button("Cancel").clicked(){self.symbol_browser.manage=None;}
        });
        if !open {
            self.symbol_browser.manage = None;
        }
    }
}
