use super::*;
use serde_json::json;
#[derive(Default)]
pub(super) struct WorkspaceUi {
    pub show: bool,
    pub reset_scroll: bool,
    pub was_shown: bool,
    pub root: Option<PathBuf>,
    pub page_dialog: bool,
    pub manager: bool,
    pub rename_path: Option<PathBuf>,
    pub rename_text: String,
    pub page_name: String,
    pub hierarchical: bool,
    pub export_dialog: bool,
    pub export_individual: bool,
    pub selected_pages: std::collections::HashSet<PathBuf>,
}
impl App {
    pub(super) fn project_overview(&mut self, ui: &mut egui::Ui) {
        if !self.reduced_motion {
            crate::design::pattern(ui, ui.max_rect());
        }
        let path = self
            .workspace_ui
            .root
            .clone()
            .or_else(|| self.doc.as_ref()?.path.clone())
            .or_else(|| self.pcb.board.as_ref()?.path.clone());
        let project = path
            .as_ref()
            .and_then(|p| jenga_tools::project_workspace::discover(p).ok());
        ui.horizontal(|ui| {
            if crate::design::button(ui, "Home", "home", false).clicked() && self.may_replace() {
                self.doc = None;
                self.pcb.board = None;
                self.pcb.active = false;
                self.workspace_ui.show = false;
            }
        });
        let mut scroll = egui::ScrollArea::vertical().id_salt(("project_workspace", &path));
        if self.workspace_ui.reset_scroll || !self.workspace_ui.was_shown {
            scroll = scroll.vertical_scroll_offset(0.);
            self.workspace_ui.reset_scroll = false;
            self.symbol_browser.open_epoch += 1;
        }
        self.workspace_ui.was_shown = true;
        scroll.show(ui,|ui|{
            ui.add_space(18.);
            crate::design::brand(ui,"PROJECT WORKSPACE",1.3);
            if let Some(project)=project {
                let scale=(ui.available_width()/(project.name.chars().count().max(1) as f32*18.)).min(2.1);
                if self.reduced_motion{crate::design::brand(ui,&project.name,scale);}else{crate::design::animated_brand(ui,&project.name,scale);}
                ui.label(project.folder.display().to_string());ui.add_space(10.);
                ui.horizontal_wrapped(|ui|{
                    if crate::design::button(ui,"Folder","folder",false).on_hover_text("Open project location").clicked(){let _=std::process::Command::new("explorer.exe").arg(&project.folder).spawn();}
if crate::design::button(ui,"New PCB","symbol",false).clicked(){let mut name="Board".to_owned();let mut count=1;while project.folder.join(format!("{name}.kicad_pcb")).exists(){count+=1;name=format!("Board{count}");}self.pcb_job("pcb_new",json!({"path":project.folder.join(format!("{name}.kicad_pcb"))}),ui.ctx());}
if self.doc.is_some() && crate::design::button(ui,"Build PCB","wire",false).on_hover_text("Audit component footprints, then preview placement and routing").clicked(){self.pcb_job("pcb_preflight",json!({}),ui.ctx());}
if crate::design::button(ui,"Footprints","symbol",false).clicked(){self.footprints.open=true;}
if crate::design::button(ui,"New schematic","symbol",false).on_hover_text("Add a separate schematic, starting with one page").clicked(){self.workspace_ui.page_dialog=true;self.workspace_ui.hierarchical=false;self.workspace_ui.page_name="NewSchematic".into();}
if crate::design::button(ui,"Add page","symbol",false).on_hover_text("Add a child page to the currently open schematic").clicked(){self.workspace_ui.page_dialog=true;self.workspace_ui.hierarchical=false;self.workspace_ui.page_name="NewPage".into();}
if crate::design::button(ui,"Page name","rename",false).on_hover_text("Edit the current schematic title and worksheet; root filenames remain paired with the KiCad project").clicked(){self.page_dialog=true;}
if crate::design::button(ui,"Export","export",false).clicked(){self.workspace_ui.export_dialog=true;self.workspace_ui.selected_pages=project.pages.iter().map(|p|p.file.clone()).collect();}
if crate::design::button(ui,"Symbol editor","symbol",false).clicked(){let mut e=symbols::SymbolEditor::default();e.set_personal(false);self.symbol_editor=Some(e);}
if crate::design::button(ui,"Rename","rename",false).clicked(){self.start_project_rename();}
if crate::design::button(ui,"Save as","save_as",false).clicked(){self.copy_project_to();}
if crate::design::button(ui,"Delete","delete",false).clicked(){self.delete_current_project();self.workspace_ui.show=self.doc.is_some();}
                });
                ui.add_space(16.);ui.label(format!("{} schematics • {} pages • native KiCad project",project.pages.iter().filter(|p|p.parent.is_none()).count(),project.pages.len()));
                let columns=if ui.available_width()<600. {1}else{2};
                let width=((ui.available_width()-18.)/columns as f32).min(520.);
                for row in project.pages.chunks(columns){
                    ui.horizontal_top(|ui|{for page in row{
                        let(rect,response)=ui.allocate_exact_size(Vec2::new(width,290.),egui::Sense::click());
                        let lift=if self.reduced_motion{0.}else{ui.ctx().animate_bool_with_time(response.id.with("page_lift"),response.hovered(),0.18)};
                        let card=rect.translate(Vec2::new(0.,-4.*lift));let painter=ui.painter_at(rect.expand(6.));
                        painter.rect_filled(card.translate(Vec2::new(0.,3.+lift*3.)),8.,Color32::from_black_alpha((10.+12.*lift)as u8));
                        painter.rect_filled(card,8.,Color32::from_rgb(253,252,248));
                        painter.rect_stroke(card,8.,Stroke::new(1_f32,if response.hovered(){ACCENT}else{Color32::from_gray(210)}),egui::StrokeKind::Inside);
                        let mut title=egui::text::LayoutJob::simple(page.name.clone(),egui::FontId::monospace(15.),Color32::from_gray(22),width-24.);title.wrap.max_rows=1;title.wrap.break_anywhere=true;title.wrap.overflow_character=Some('…');painter.galley(card.left_top()+Vec2::new(12.,8.),painter.layout_job(title),Color32::from_gray(22));
                        painter.text(card.left_top()+Vec2::new(12.,39.),egui::Align2::LEFT_CENTER,if page.parent.is_some(){"Hierarchical child · linked through sheet pins"}else{"Independent drawing · separate electrical scope"},egui::FontId::monospace(11.),Color32::from_gray(90));
                        if let Ok(doc)=Document::open(&page.file){let scene=Scene::from_document(&doc);let preview=egui::Rect::from_min_size(card.min+Vec2::new(12.,55.),Vec2::new(width-24.,150.));let mut view=View{show_grid:false,..Default::default()};view.fit(&scene,preview);canvas::paint(&ui.painter_at(preview),&view,preview,&scene,None,false);
                            painter.text(card.left_bottom()+Vec2::new(12.,-75.),egui::Align2::LEFT_CENTER,format!("{} components",doc.root.children("symbol").count()),egui::FontId::monospace(11.),Color32::from_gray(70));
                        }
                        let mut action=None;
                        let area=egui::Rect::from_min_size(card.left_bottom()+Vec2::new(10.,-59.),Vec2::new(width-20.,49.));
                        let mut buttons=ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::left_to_right(egui::Align::Min)));buttons.set_clip_rect(area);
                        for(label,icon)in[("Open","select"),("Rename","rename"),("Export","export"),("Delete","delete")]{if crate::design::button(&mut buttons,label,icon,false).clicked(){action=Some(label);}}
if let Some(action)=action{self.open_path(&page.file);match action{
                            "Open"=>self.workspace_ui.show=false,
                            "Rename"=>self.page_dialog=true,
                            "Export"=>{self.workspace_ui.export_dialog=true;self.workspace_ui.selected_pages=std::iter::once(page.file.clone()).collect();},
                            "Delete"=>{if project.project_file.as_ref().is_some_and(|p|p.with_extension("kicad_sch")==page.file){self.delete_current_project();}else if rfd::MessageDialog::new().set_title("Delete page?").set_description("Move this page to recovery storage and remove its parent sheet link?").set_buttons(rfd::MessageButtons::OkCancel).show()==rfd::MessageDialogResult::Ok&&self.save(false){match jenga_tools::project_workspace::delete_page(self.doc.as_ref().unwrap(),&json!({"path":page.file})){Ok(_)=>{let root=project.project_file.as_ref().map(|p|p.with_extension("kicad_sch"));self.doc=None;if let Some(root)=root{self.open_path(&root);}self.status="Page deleted · recoverable".into();},Err(e)=>self.fail(e)}}},_=>{}}
                        }
if response.on_hover_text(page.file.display().to_string()).clicked(){self.open_path(&page.file);self.workspace_ui.show=false;}
                    }});ui.add_space(12.);
                }
                ui.separator();ui.heading("Printed circuit boards");
                ui.horizontal_wrapped(|ui|{for file in &project.boards{egui::Frame::group(ui.style()).show(ui,|ui|{ui.set_width(360.);ui.strong(file.file_stem().unwrap_or_default().to_string_lossy());if let Ok(board)=jenga_core::pcb::Board::open(file){let scene=board.scene(None,false);let(rect,r)=ui.allocate_exact_size(Vec2::new(340.,170.),egui::Sense::click());let mut view=View{show_grid:false,..Default::default()};view.fit(&scene,rect);canvas::paint(&ui.painter_at(rect),&view,rect,&scene,None,false);ui.label(format!("{} footprints · {}",board.root.children("footprint").count(),if board.schematic.is_some(){"Linked schematic"}else{"Independent PCB"}));if (r.clicked()||ui.button("Open PCB").clicked())&& self.may_replace(){self.open_path(file);self.workspace_ui.show=false;}}});}});
                ui.separator();ui.heading("Project footprints");self.project_footprint_cards(ui,&project.folder);
                ui.separator();ui.label("Project symbols");
                if ui.button("Browse / edit project symbols…").clicked(){self.symbol_browser.open=true;self.symbol_browser.project_only=true;self.symbol_browser.open_epoch+=1;}
                ui.label("Create custom symbols here or promote them to your global personal library.");
                self.project_symbol_cards(ui,&project.folder);
                ui.separator();ui.label("Project files and libraries");
                if let Ok(files)=std::fs::read_dir(&project.folder){for file in files.flatten().filter(|f|f.path().extension().is_some_and(|e|e=="kicad_pro"||e=="kicad_pcb")||f.file_name()=="libraries"||f.file_name()=="sym-lib-table"||f.file_name()=="fp-lib-table"){ui.label(file.file_name().to_string_lossy());}}
                ui.label("Symbol → footprint links use native KiCad library IDs. 3D models belong to footprints.");
            }else{ui.label("Save a schematic or open a project to see its workspace.");if ui.button("Return to editor").clicked(){self.workspace_ui.show=false;}}
        });
    }
    pub(super) fn start_project_rename(&mut self) {
        if self.pcb.active {
            if self.save_pcb() {
                self.rename_name = self
                    .pcb
                    .board
                    .as_ref()
                    .and_then(|b| b.path.as_ref())
                    .and_then(|p| p.file_stem())
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.rename_dialog = true;
                self.rename_folder = false;
            }
            return;
        }
        if self.doc.is_some() && self.save(false) {
            if let Some(path) = self.doc.as_ref().and_then(|d| d.path.clone())
                && let Ok(project) = jenga_tools::project_workspace::discover(&path)
                && let Some(pro) = project.project_file
            {
                let root = pro.with_extension("kicad_sch");
                if root.is_file() && !jenga_tools::catalog::same_path(&root, &path) {
                    self.open_path(&root);
                }
            }
            let d = self.doc.as_ref().unwrap();
            self.rename_name = d.project_name();
            self.rename_project = d
                .path
                .as_ref()
                .is_some_and(|p| p.with_extension("kicad_pro").exists());
            self.rename_folder = false;
            self.rename_dialog = true;
        }
    }
    pub(super) fn copy_project_to(&mut self) {
        if self.pcb.active {
            if self.save_pcb()
                && let Some(folder) = rfd::FileDialog::new().pick_folder()
            {
                let name = format!(
                    "{}Copy",
                    self.pcb
                        .board
                        .as_ref()
                        .and_then(|b| b.path.as_ref())
                        .and_then(|p| p.file_stem())
                        .unwrap_or_default()
                        .to_string_lossy()
                );
                if let Err(e) = self.control_request(
                    "pcb_copy_project",
                    &json!({"folder":folder,"name":name,"actor":"user"}),
                ) {
                    self.fail(e);
                }
            }
            return;
        }
        if self.save(false)
            && let Some(folder) = rfd::FileDialog::new().pick_folder()
        {
            let name = self.doc.as_ref().unwrap().project_name();
            if let Err(e) = self.control_request(
                "save_project_as",
                &json!({"folder":folder,"name":name,"actor":"user"}),
            ) {
                self.fail(e);
            }
        }
    }
    pub(super) fn workspace_dialogs(&mut self, ctx: &egui::Context) {
        if self.workspace_ui.page_dialog {
            let mut open = true;
            egui::Window::new(if self.workspace_ui.hierarchical{"Add page to current schematic"}else{"Add schematic to project"}).open(&mut open).show(ctx,|ui|{
                ui.label("Name");ui.text_edit_singleline(&mut self.workspace_ui.page_name);ui.checkbox(&mut self.workspace_ui.hierarchical,"Electrically linked hierarchical child (adds a sheet rectangle to parent)");ui.label(if self.workspace_ui.hierarchical{"Native child sheet of the current schematic"}else{"Separate schematic with one page"});
                ui.label("Independent schematics and child pages both stay inside this project.");
                if ui.button("Create page").clicked()&&self.save(false){let p=json!({"name":self.workspace_ui.page_name,"hierarchical":self.workspace_ui.hierarchical,"actor":"user"});match self.control_request("create_page",&p){Ok(result)=>{self.workspace_ui.page_dialog=false;if let Some(path)=result["file"].as_str(){self.open_path(&PathBuf::from(path));self.workspace_ui.show=false;}self.status="Created page · ready to edit".into();},Err(e)=>self.fail(e)}}
            });
            self.workspace_ui.page_dialog &= open;
        }
        if self.workspace_ui.export_dialog {
            let mut open = true;
            egui::Window::new("Export selected project pages").open(&mut open).show(ctx,|ui|{
                let project=self.workspace_ui.root.as_ref().or_else(||self.doc.as_ref().and_then(|d|d.path.as_ref())).and_then(|p|jenga_tools::project_workspace::discover(p).ok());
                if let Some(project)=project{
                    ui.horizontal(|ui|{if ui.button("All pages").clicked(){self.workspace_ui.selected_pages=project.pages.iter().map(|p|p.file.clone()).collect();}
if ui.button("Current page only").clicked(){self.workspace_ui.selected_pages=self.doc.as_ref().and_then(|d|d.path.clone()).into_iter().collect();}});
                    for page in &project.pages{let mut selected=self.workspace_ui.selected_pages.contains(&page.file);if ui.checkbox(&mut selected,&page.name).changed(){if selected{self.workspace_ui.selected_pages.insert(page.file.clone());}else{self.workspace_ui.selected_pages.remove(&page.file);}}}
                    ui.checkbox(&mut self.workspace_ui.export_individual,"Export a separate file for each page (including PDF)");ui.label("With separate files off, PDF combines selected pages. SVG/PNG/native always export individual files.");
                    ui.horizontal(|ui|{for format in ["pdf","png","svg","kicad_sch"]{if ui.button(format.to_uppercase()).clicked()&&self.save(false){let path=if format=="pdf"&&!self.workspace_ui.export_individual{rfd::FileDialog::new().set_file_name(format!("{}.pdf",project.name)).save_file()}else{rfd::FileDialog::new().pick_folder()};if let Some(path)=path{let p=json!({"path":path,"format":format,"separate":self.workspace_ui.export_individual,"pages":self.workspace_ui.selected_pages,"actor":"user"});match jenga_tools::project_workspace::export_pages(&project,&p){Ok(_)=>{self.status="Exported selected project pages".into();self.workspace_ui.export_dialog=false;},Err(e)=>self.fail(e)}}}}});
                }
            });
            self.workspace_ui.export_dialog &= open;
        }
        self.page_manager(ctx);
        self.symbol_editor_dialog(ctx);
        self.symbol_browser_dialog(ctx);
        self.delete_dialog(ctx);
        self.pcb_dialogs(ctx);
    }
}

impl App {
    pub(super) fn page_manager(&mut self, ctx: &egui::Context) {
        if !self.workspace_ui.manager {
            return;
        }
        let mut open = true;
        egui::Window::new("Pages · switch and organise").open(&mut open).default_width(760.).vscroll(true).show(ctx,|ui|{
   ui.label("Switch pages, edit their worksheet/title, rename or reorder sibling pages. Root schematics remain first.");
   ui.horizontal(|ui|{if ui.button("Add page").clicked(){self.workspace_ui.page_dialog=true;self.workspace_ui.hierarchical=false;self.workspace_ui.page_name="NewPage".into();}
if ui.button("Export pages…").clicked(){self.workspace_ui.export_dialog=true;}});
   if let Some(path)=self.doc.as_ref().and_then(|d|d.path.as_ref())&&let Ok(project)=jenga_tools::project_workspace::discover(path){
    for(i,page)in project.pages.iter().enumerate(){ui.horizontal_wrapped(|ui|{
     let active=self.doc.as_ref().and_then(|d|d.path.as_ref())==Some(&page.file);
     if ui.selectable_label(active,format!("{}  {}",i+1,page.name)).on_hover_text(page.file.display().to_string()).clicked(){self.open_path(&page.file);}
if ui.small_button("Title / edit").clicked(){self.open_path(&page.file);self.page_dialog=true;}
if ui.small_button("Rename").clicked(){self.workspace_ui.rename_path=Some(page.file.clone());self.workspace_ui.rename_text=page.name.clone();}
     for(label,up)in[("↑",true),("↓",false)]{let other=if up{(0..i).rev().find(|&j|project.pages[j].parent==page.parent)}else{(i+1..project.pages.len()).find(|&j|project.pages[j].parent==page.parent)};
      if ui.add_enabled(page.parent.is_some()&&other.is_some(),egui::Button::new(label)).clicked()&&self.save(false){let order=match jenga_tools::project_workspace::move_sibling(&project,&page.file,up){Ok(v)=>v,Err(e)=>{self.fail(e);return;}};if let Err(e)=self.control_request("reorder_pages",&json!({"pages":order,"actor":"user"})){self.fail(e);}}
     }
if ui.small_button("Delete").clicked(){if project.project_file.as_ref().is_some_and(|p|p.with_extension("kicad_sch")==page.file){self.open_path(&page.file);self.delete_current_project();}else if rfd::MessageDialog::new().set_title("Delete page?").set_description("Archive this page and remove its parent sheet link? It remains recoverable.").set_buttons(rfd::MessageButtons::OkCancel).show()==rfd::MessageDialogResult::Ok&&self.save(false)&& let Err(e)=self.control_request("delete_page",&json!({"path":page.file,"actor":"user"})){self.fail(e);}}
    });}
   }
if let Some(path)=self.workspace_ui.rename_path.clone(){ui.separator();ui.text_edit_singleline(&mut self.workspace_ui.rename_text);ui.horizontal(|ui|{if ui.button("Save page name").clicked()&&self.save(false){let name=self.workspace_ui.rename_text.clone();match self.control_request("rename_page",&json!({"path":path,"name":name,"actor":"user"})){Ok(_)=>self.workspace_ui.rename_path=None,Err(e)=>self.fail(e)}}
if ui.button("Cancel rename").clicked(){self.workspace_ui.rename_path=None;}});}
  });
        self.workspace_ui.manager = open;
    }
}
