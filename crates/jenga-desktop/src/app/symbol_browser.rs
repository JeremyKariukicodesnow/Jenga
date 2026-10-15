use super::*;
use serde_json::json;
#[derive(Default)]
pub(super) struct SymbolBrowser {
    pub open: bool,
    pub project_only: bool,
    query: String,
    manage: Option<(PathBuf, String, String)>,
    rename: String,
    checked: u128,
    signature: String,
    selected: Option<(String, PathBuf)>,
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
    pub(super) fn project_symbol_cards(&mut self, ui: &mut egui::Ui, folder: &std::path::Path) {
        let mut any = false;
        for (_, path) in self
            .custom_sources()
            .into_iter()
            .filter(|(_, p)| p.starts_with(folder))
        {
            if let Ok(text) = std::fs::read_to_string(&path)
                && let Ok(root) = jenga_core::sexpr::parse(&text)
            {
                any |= root.children("symbol").next().is_some();
                egui::CollapsingHeader::new(path.file_stem().unwrap_or_default().to_string_lossy())
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for node in root.children("symbol").take(12) {
                                egui::Frame::group(ui.style()).show(ui, |ui| {
                                    ui.set_width(230.);
                                    ui.label(RichText::new(node.value(1)).strong());
                                    if let Ok(def) =
                                        Libraries::symbol_from_file(&path, node.value(1))
                                    {
                                        let scene = Scene::symbol_preview_unit(&def, 1);
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(230., 100.),
                                            egui::Sense::hover(),
                                        );
                                        let mut view = View {
                                            show_grid: false,
                                            ..Default::default()
                                        };
                                        view.fit(&scene, rect);
                                        canvas::paint(
                                            &ui.painter_at(rect),
                                            &view,
                                            rect,
                                            &scene,
                                            None,
                                            false,
                                        );
                                    }
                                    ui.horizontal(|ui| {
                                        if crate::design::button(ui, "Edit", "symbol", false)
                                            .clicked()
                                        {
                                            self.open_library_symbol(&path, node.value(1), false);
                                        }
                                        if crate::design::button(ui, "Global", "project", false)
                                            .clicked()
                                        {
                                            self.promote_symbol(&path, node.value(1));
                                        }
                                        ui.menu_button("⋯", |ui| {
                                            for action in ["rename", "export", "delete"] {
                                                if ui.button(action).clicked() {
                                                    self.symbol_browser.manage = Some((
                                                        path.clone(),
                                                        node.value(1).into(),
                                                        action.into(),
                                                    ));
                                                    self.symbol_browser.rename =
                                                        node.value(1).into();
                                                    ui.close();
                                                }
                                            }
                                        });
                                    });
                                });
                            }
                        });
                    });
            }
        }
        if !any {
            ui.label("No custom project symbols yet. Use Symbol editor to create one.");
        }
    }
    fn open_library_symbol(&mut self, path: &std::path::Path, name: &str, personal: bool) {
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
    fn promote_symbol(&mut self, path: &std::path::Path, name: &str) {
        let result = (|| -> anyhow::Result<serde_json::Value> {
            let def = Libraries::symbol_from_file(path, name)?;
            let personal = jenga_tools::symbol_editor::personal_folder()?
                .join("libraries/SharedSymbols.kicad_sym");
            let mut p = json!({"name":name,"library":"SharedSymbols","scope":"personal","native_source":def.encoded(),"license":"Original source library license applies; not relicensed","attribution":path.display().to_string(),"actor":"user"});
            if personal.exists() {
                let text = std::fs::read_to_string(&personal)?;
                let root = jenga_core::sexpr::parse(&text)?;
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
            return;
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
            if self.symbol_browser.selected.is_none() {
                if let Some(e) = self
                    .libraries
                    .as_ref()
                    .and_then(|l| l.entries.iter().find(|e| e.id == "Device:R"))
                {
                    self.symbol_browser.selected = Some(("R".into(), e.file.clone()));
                }
            }
        }
        let mut open = true;
        egui::Window::new("Symbols · libraries and your projects").open(&mut open).default_size([940.,650.]).vscroll(true).show(ctx,|ui|{
            ui.horizontal_wrapped(|ui|{
                if ui.button("Create global symbol").clicked(){self.symbol_editor=Some(Default::default());}
                if self.doc.is_some()&&ui.button("Create project symbol").clicked(){let mut e=symbols::SymbolEditor::default();e.set_personal(false);self.symbol_editor=Some(e);}
                ui.checkbox(&mut self.symbol_browser.project_only,"Custom symbols only");
            });
            ui.add(egui::TextEdit::singleline(&mut self.symbol_browser.query).hint_text("Search names, descriptions and keywords…").desired_width(f32::INFINITY));
            ui.label("Edit creates a personal/project derivative; installed KiCad libraries are preserved. Make global shares a custom symbol across projects.");
            let sources=self.custom_sources();let query=self.symbol_browser.query.to_lowercase();
            egui::ScrollArea::vertical().max_height(270.).show(ui,|ui|{
                for(group,path)in sources{
                    if let Ok(text)=std::fs::read_to_string(&path)&&let Ok(root)=jenga_core::sexpr::parse(&text){
                        ui.collapsing(format!("{group} / {}",path.file_stem().unwrap_or_default().to_string_lossy()),|ui|{
                            for n in root.children("symbol"){if !format!("{} {}",n.value(1),property(n,"Description")).to_lowercase().contains(&query){continue;}
                                ui.horizontal(|ui|{if ui.selectable_label(self.symbol_browser.selected.as_ref().is_some_and(|(name,file)|name==n.value(1)&&file==&path),n.value(1)).clicked(){self.symbol_browser.selected=Some((n.value(1).into(),path.clone()));}
                                    if ui.small_button("Edit").clicked(){self.open_library_symbol(&path,n.value(1),group.starts_with("Global"));}
                                    if !group.starts_with("Global")&&ui.small_button("Make global").clicked(){self.promote_symbol(&path,n.value(1));}ui.menu_button("⋯",|ui|{for action in ["rename","export","delete"]{if ui.button(action).clicked(){self.symbol_browser.manage=Some((path.clone(),n.value(1).into(),action.into()));self.symbol_browser.rename=n.value(1).into();ui.close();}}});
                                });
                            }
                        });
                    }
                }
                if !self.symbol_browser.project_only{
                    if let Some(libs)=&self.libraries{let entries:Vec<_>=libs.search(&self.symbol_browser.query,"").into_iter().take(300).map(|e|(e.id.clone(),e.file.clone(),e.description.clone())).collect();for(id,path,desc)in entries{ui.horizontal(|ui|{if ui.selectable_label(false,&id).on_hover_text(&desc).clicked(){self.symbol_browser.selected=Some((id.split_once(':').unwrap().1.into(),path.clone()));}if ui.small_button("Edit derivative").clicked(){self.open_library_symbol(&path,id.split_once(':').unwrap().1,true);}});}}
                    else{ui.spinner();ui.label("Indexing library symbols…");}
                }
            });
            if let Some((name,path))=self.symbol_browser.selected.clone(){
                ui.separator();ui.heading(&name);ui.label(path.display().to_string());
                if let Ok(def)=Libraries::symbol_from_file(&path,&name){let scene=Scene::symbol_preview_unit(&def,1);let(rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),200.),egui::Sense::hover());let mut view=View{show_grid:false,..Default::default()};view.fit(&scene,rect);canvas::paint(&ui.painter_at(rect),&view,rect,&scene,None,false);}
            }
        });
        self.symbol_browser.open = open;
    }
}

impl App {
    fn symbol_manage_dialog(&mut self, ctx: &egui::Context) {
        let Some((path, name, action)) = self.symbol_browser.manage.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new(format!("{action} symbol · {name}")).open(&mut open).collapsible(false).show(ctx,|ui|{
   if action=="rename"{ui.text_edit_singleline(&mut self.symbol_browser.rename);}ui.label("Placed schematic instances keep their embedded definition. Library changes are backed up.");
   if ui.button("Confirm").clicked(){let result=(||->anyhow::Result<serde_json::Value>{let root=jenga_core::sexpr::parse(&std::fs::read_to_string(&path)?)?;let base=path.parent().and_then(|p|p.parent()).unwrap();let personal=jenga_tools::symbol_editor::personal_folder().is_ok_and(|p|p==base);let mut context=Document::new();context.path=Some(base.join("context.kicad_sch"));let mut p=json!({"library":path.file_stem().unwrap().to_string_lossy(),"scope":if personal{"personal"}else{"project"},"name":name,"action":action,"new_name":self.symbol_browser.rename,"expected_revision":jenga_tools::journal::revision_text(&root.encoded())});if action=="export"{let out=rfd::FileDialog::new().set_file_name(format!("{name}.kicad_sym")).save_file().ok_or_else(||anyhow::anyhow!("Export cancelled"))?;p["path"]=json!(out);}jenga_tools::library_manage::manage(Some(&context),&p)})();match result{Ok(_)=>{self.symbol_browser.manage=None;self.reload_libraries();self.status=format!("Symbol {action} complete");},Err(e)=>self.fail(e)}}
   if ui.button("Cancel").clicked(){self.symbol_browser.manage=None;}
  });
        if !open {
            self.symbol_browser.manage = None;
        }
    }
}
