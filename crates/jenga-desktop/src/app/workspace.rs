use super::*;
use serde_json::json;
#[derive(Default)]
pub(super) struct WorkspaceUi {
    pub show: bool,
    pub root: Option<PathBuf>,
    pub page_dialog: bool,
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
            .or_else(|| self.doc.as_ref()?.path.clone());
        let project = path
            .as_ref()
            .and_then(|p| jenga_tools::project_workspace::discover(p).ok());
        egui::ScrollArea::vertical().show(ui,|ui|{
            ui.add_space(18.);
            if self.reduced_motion{crate::design::brand(ui,"PROJECT WORKSPACE",1.3);}else{crate::design::animated_brand(ui,"PROJECT WORKSPACE",1.3);}
            if let Some(project)=project {
                ui.heading(RichText::new(&project.name).color(Color32::from_gray(20)).size(26.));
                ui.label(project.folder.display().to_string());ui.add_space(10.);
                ui.horizontal_wrapped(|ui|{
                    if crate::design::button(ui,"New schematic","symbol",false).on_hover_text("Add a separate schematic, starting with one page").clicked(){self.workspace_ui.page_dialog=true;self.workspace_ui.hierarchical=false;self.workspace_ui.page_name="NewSchematic".into();}
                    if crate::design::button(ui,"Add page","symbol",false).on_hover_text("Add a child page to the currently open schematic").clicked(){self.workspace_ui.page_dialog=true;self.workspace_ui.hierarchical=true;self.workspace_ui.page_name="NewPage".into();}
                    if crate::design::button(ui,"Page name","rename",false).on_hover_text("Edit the current schematic title and worksheet; root filenames remain paired with the KiCad project").clicked(){self.page_dialog=true;}
                    if crate::design::button(ui,"Export","export",false).clicked(){self.workspace_ui.export_dialog=true;self.workspace_ui.selected_pages=project.pages.iter().map(|p|p.file.clone()).collect();}
                    if crate::design::button(ui,"Symbols","symbol",false).clicked(){self.symbol_editor=Some(Default::default());}
                    if crate::design::button(ui,"Rename","rename",false).clicked(){self.start_project_rename();}
                    if crate::design::button(ui,"Save as","save_as",false).clicked(){self.copy_project_to();}
                    if crate::design::button(ui,"Delete","delete",false).clicked(){self.delete_current_project();self.workspace_ui.show=self.doc.is_some();}
                });
                ui.add_space(16.);ui.label(format!("{} schematics • {} pages • native KiCad project",project.pages.iter().filter(|p|p.parent.is_none()).count(),project.pages.len()));
                let width=((ui.available_width()-18.)/2.).clamp(240.,520.);
                for row in project.pages.chunks(2){
                    ui.horizontal(|ui|{for page in row{
                        let(rect,response)=ui.allocate_exact_size(Vec2::new(width,245.),egui::Sense::click());
                        let lift=if self.reduced_motion{0.}else{ui.ctx().animate_bool_with_time(response.id.with("page_lift"),response.hovered(),0.18)};
                        let card=rect.translate(Vec2::new(0.,-4.*lift));let painter=ui.painter_at(rect.expand(6.));
                        painter.rect_filled(card.translate(Vec2::new(0.,3.+lift*3.)),8.,Color32::from_black_alpha((10.+12.*lift)as u8));
                        painter.rect_filled(card,8.,Color32::from_rgb(253,252,248));
                        painter.rect_stroke(card,8.,Stroke::new(1_f32,if response.hovered(){ACCENT}else{Color32::from_gray(210)}),egui::StrokeKind::Inside);
                        painter.text(card.left_top()+Vec2::new(12.,18.),egui::Align2::LEFT_CENTER,&page.name,egui::FontId::monospace(15.),Color32::from_gray(22));
                        painter.text(card.left_top()+Vec2::new(12.,39.),egui::Align2::LEFT_CENTER,if page.parent.is_some(){"Hierarchical sheet"}else{"Schematic"},egui::FontId::monospace(11.),Color32::from_gray(90));
                        if let Ok(doc)=Document::open(&page.file){let scene=Scene::from_document(&doc);let preview=egui::Rect::from_min_size(card.min+Vec2::new(12.,55.),Vec2::new(width-24.,150.));let mut view=View{show_grid:false,..Default::default()};view.fit(&scene,preview);canvas::paint(&ui.painter_at(preview),&view,preview,&scene,None,false);
                            painter.text(card.left_bottom()+Vec2::new(12.,-17.),egui::Align2::LEFT_CENTER,format!("{} components",doc.root.children("symbol").count()),egui::FontId::monospace(11.),Color32::from_gray(70));
                        }
                        painter.text(card.right_bottom()+Vec2::new(-12.,-17.),egui::Align2::RIGHT_CENTER,"Open page →",egui::FontId::monospace(12.),Color32::from_gray(22));
                        if response.on_hover_text(page.file.display().to_string()).clicked(){self.open_path(&page.file);self.workspace_ui.show=false;}
                    }});ui.add_space(12.);
                }
                ui.separator();ui.label("Project files and libraries");
                if let Ok(files)=std::fs::read_dir(&project.folder){for file in files.flatten().filter(|f|f.path().extension().is_some_and(|e|e=="kicad_pro"||e=="kicad_pcb")||f.file_name()=="libraries"||f.file_name()=="sym-lib-table"||f.file_name()=="fp-lib-table"){ui.label(file.file_name().to_string_lossy());}}
                ui.label("Symbol → footprint links use native KiCad library IDs. 3D models belong to footprints.");
            }else{ui.label("Save a schematic or open a project to see its workspace.");if ui.button("Return to editor").clicked(){self.workspace_ui.show=false;}}
        });
    }
    pub(super) fn start_project_rename(&mut self) {
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
                ui.label("Name");ui.text_edit_singleline(&mut self.workspace_ui.page_name);ui.label(if self.workspace_ui.hierarchical{"Native child sheet of the current schematic"}else{"Separate schematic with one page"});
                ui.label("Independent schematics and child pages both stay inside this project.");
                if ui.button("Create page").clicked()&&self.save(false){let p=json!({"name":self.workspace_ui.page_name,"hierarchical":self.workspace_ui.hierarchical,"actor":"user"});match self.control_request("create_page",&p){Ok(_)=>{self.workspace_ui.page_dialog=false;self.status="Created native KiCad page".into();},Err(e)=>self.fail(e)}}
            });
            self.workspace_ui.page_dialog &= open;
        }
        if self.workspace_ui.export_dialog {
            let mut open = true;
            egui::Window::new("Export selected project pages").open(&mut open).show(ctx,|ui|{
                let project=self.workspace_ui.root.as_ref().or_else(||self.doc.as_ref().and_then(|d|d.path.as_ref())).and_then(|p|jenga_tools::project_workspace::discover(p).ok());
                if let Some(project)=project{
                    ui.horizontal(|ui|{if ui.button("All pages").clicked(){self.workspace_ui.selected_pages=project.pages.iter().map(|p|p.file.clone()).collect();}if ui.button("Current page only").clicked(){self.workspace_ui.selected_pages=self.doc.as_ref().and_then(|d|d.path.clone()).into_iter().collect();}});
                    for page in &project.pages{let mut selected=self.workspace_ui.selected_pages.contains(&page.file);if ui.checkbox(&mut selected,&page.name).changed(){if selected{self.workspace_ui.selected_pages.insert(page.file.clone());}else{self.workspace_ui.selected_pages.remove(&page.file);}}}
                    ui.checkbox(&mut self.workspace_ui.export_individual,"Export a separate file for each page (including PDF)");ui.label("With separate files off, PDF combines selected pages. SVG/PNG/native always export individual files.");
                    ui.horizontal(|ui|{for format in ["pdf","png","svg","kicad_sch"]{if ui.button(format.to_uppercase()).clicked()&&self.save(false){let path=if format=="pdf"&&!self.workspace_ui.export_individual{rfd::FileDialog::new().set_file_name(format!("{}.pdf",project.name)).save_file()}else{rfd::FileDialog::new().pick_folder()};if let Some(path)=path{let p=json!({"path":path,"format":format,"separate":self.workspace_ui.export_individual,"pages":self.workspace_ui.selected_pages,"actor":"user"});match jenga_tools::project_workspace::export_pages(&project,&p){Ok(_)=>{self.status="Exported selected project pages".into();self.workspace_ui.export_dialog=false;},Err(e)=>self.fail(e)}}}}});
                }
            });
            self.workspace_ui.export_dialog &= open;
        }
        self.symbol_editor_dialog(ctx);
    }
}
