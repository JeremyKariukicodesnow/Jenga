use super::*;
use anyhow::Context;
use jenga_core::{pcb, sexpr::parse};
use serde_json::json;
#[derive(Default)]
pub(super) struct Footprints {
    pub open: bool,
    pub preview_mesh: Option<jenga_core::mesh::Mesh>,
    pub preview_open: bool,
    yaw: f64,
    pitch: f64,
    zoom: f32,
    pan: Vec2,
    custom_only: bool,
    search: String,
    selected: Option<String>,
    editor: Option<FootprintEditor>,
    sources: Vec<(String, PathBuf)>,
    loaded: bool,
    epoch: u64,
    body_size: [f64; 3],
    body_cylinder: bool,
}
struct FootprintEditor {
    project_folder: Option<PathBuf>,
    source: Node,
    native: String,
    name: String,
    library: String,
    scope: String,
    revision: Option<String>,
    pad_number: String,
    model_path: String,
    model_offset: [f64; 3],
    model_scale: [f64; 3],
    model_rotation: [f64; 3],
    graphic: String,
    first: Option<Point>,
}
impl App {
    pub(super) fn dismiss_footprints(&mut self) {
        self.footprints.open = false;
        self.footprints.editor = None;
        self.footprints.loaded = false;
        self.footprints.preview_open = false;
    }
    pub(super) fn load_footprint_editor_file(&mut self, path: &Path) {
        let id = format!(
            "{}:{}",
            path.parent()
                .and_then(|p| p.file_stem())
                .unwrap_or_default()
                .to_string_lossy(),
            path.file_stem().unwrap_or_default().to_string_lossy()
        );
        self.load_footprint_editor(&id, Some(path));
    }
    pub(super) fn refresh_footprint_sources(&mut self) {
        self.footprints.sources = self.footprint_sources();
    }
    pub(super) fn footprint_sources(&self) -> Vec<(String, PathBuf)> {
        let mut out = vec![];
        let mut folders: Vec<_> = self
            .settings
            .recent
            .iter()
            .filter_map(|p| p.parent().map(Path::to_owned))
            .collect();
        if let Some(p) = self
            .pcb
            .board
            .as_ref()
            .and_then(|b| b.path.as_ref())
            .or_else(|| self.doc.as_ref()?.path.as_ref())
            .and_then(|p| p.parent())
        {
            folders.push(p.to_owned());
        }
        folders.sort();
        folders.dedup();
        for folder in folders {
            if let Ok(entries) = std::fs::read_dir(folder.join("libraries")) {
                for file in entries
                    .flatten()
                    .filter(|e| e.path().extension().is_some_and(|e| e == "pretty"))
                {
                    out.push((
                        format!(
                            "Project · {}",
                            folder.file_name().unwrap_or_default().to_string_lossy()
                        ),
                        file.path(),
                    ));
                }
            }
        }
        if let Ok(folder) = jenga_tools::symbol_editor::personal_folder() {
            if let Ok(entries) = std::fs::read_dir(folder.join("libraries")) {
                for file in entries
                    .flatten()
                    .filter(|e| e.path().extension().is_some_and(|e| e == "pretty"))
                {
                    out.push(("Global personal".into(), file.path()));
                }
            }
        }
        out
    }
    fn load_footprint_editor(&mut self, id: &str, path: Option<&Path>) {
        let result = if let Some(path) = path {
            std::fs::read_to_string(path)
                .map_err(anyhow::Error::from)
                .and_then(|s| parse(&s))
        } else {
            self.libraries
                .as_ref()
                .context("Libraries still indexing")
                .and_then(|l| l.footprint(id))
        };
        match result {
            Ok(def) => {
                let (alias, name) = id.split_once(':').unwrap_or(("Custom", id));
                let personal = path.is_none()
                    || path.is_some_and(|p| {
                        jenga_tools::symbol_editor::personal_folder()
                            .is_ok_and(|d| p.starts_with(d))
                    });
                self.footprints.editor = Some(FootprintEditor {
                    project_folder: path
                        .and_then(|p| p.parent()?.parent()?.parent())
                        .map(Path::to_owned),
                    native: def.document_text(),
                    revision: path.map(|_| jenga_tools::journal::revision_text(&def.encoded())),
                    source: def,
                    name: name.into(),
                    library: if path.is_none() {
                        "Custom".into()
                    } else {
                        alias.into()
                    },
                    scope: if personal { "personal" } else { "project" }.into(),
                    pad_number: "1".into(),
                    model_path: String::new(),
                    model_offset: [0.; 3],
                    model_scale: [1.; 3],
                    model_rotation: [0.; 3],
                    graphic: "Select".into(),
                    first: None,
                });
            }
            Err(e) => self.fail(e),
        }
    }
    pub(super) fn project_footprint_cards(&mut self, ui: &mut egui::Ui, folder: &Path) {
        let sources: Vec<_> = self
            .footprint_sources()
            .into_iter()
            .filter(|(_, p)| p.starts_with(folder))
            .collect();
        for (_, path) in sources {
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            egui::CollapsingHeader::new(format!("Custom footprint library · {name}"))
                .default_open(true)
                .show(ui, |ui| self.footprint_cards(ui, &path));
        }
    }
    fn footprint_cards(&mut self, ui: &mut egui::Ui, path: &Path) {
        let mut files: Vec<_> = std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|e| e == "kicad_mod")
                    && p.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&self.footprints.search.to_lowercase())
            })
            .collect();
        files.sort();
        let columns = (ui.available_width() / 200.).floor().max(1.) as usize;
        let width = (ui.available_width() / columns as f32 - 10.).min(270.);
        for row in files.chunks(columns) {
            ui.horizontal_top(|ui| {
                for file in row {
                    let name = file.file_stem().unwrap_or_default().to_string_lossy();
                    let id = format!(
                        "{}:{name}",
                        path.file_stem().unwrap_or_default().to_string_lossy()
                    );
                    let (rect, r) =
                        ui.allocate_exact_size(Vec2::new(width, 170.), egui::Sense::click());
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, 8., Color32::from_rgb(253, 252, 248));
                    painter.rect_stroke(
                        rect,
                        8.,
                        Stroke::new(
                            1_f32,
                            if r.hovered() {
                                ACCENT
                            } else {
                                Color32::from_gray(200)
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        rect.left_top() + Vec2::new(10., 12.),
                        egui::Align2::LEFT_TOP,
                        name.as_ref(),
                        egui::FontId::monospace(13.),
                        Color32::BLACK,
                    );
                    if let Ok(s) = std::fs::read_to_string(file)
                        && let Ok(def) = parse(&s)
                    {
                        let scene = Scene::footprint_preview(&def);
                        let preview = rect.shrink2(Vec2::new(10., 32.));
                        let mut view = View {
                            show_grid: false,
                            ..Default::default()
                        };
                        view.fit_resource(&scene, preview);
                        canvas::paint(&painter, &view, preview, &scene, None, false);
                        painter.text(
                            rect.left_bottom() + Vec2::new(10., -10.),
                            egui::Align2::LEFT_BOTTOM,
                            format!(
                                "{} pads · {} models",
                                def.children("pad").count(),
                                def.children("model").count()
                            ),
                            egui::FontId::monospace(10.),
                            Color32::from_gray(80),
                        );
                    }
                    if r.hovered() {
                        self.footprints.selected = Some(id.clone());
                    }
                    if r.on_hover_text(file.display().to_string()).clicked() {
                        self.load_footprint_editor(&id, Some(file));
                    }
                }
            });
            ui.add_space(10.);
        }
    }
    pub(super) fn footprint_dialog(&mut self, ctx: &egui::Context) {
        if self.footprints.preview_open {
            let mut open = true;
            egui::Window::new("Footprint 3D model alignment")
                .open(&mut open)
                .default_size(Vec2::new(850., 600.))
                .show(ctx, |ui| {
                    ui.label("Drag to orbit · right drag to pan · wheel to zoom");
                    if let Some(mesh) = &self.footprints.preview_mesh {
                        let (rect, r) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 500.),
                            egui::Sense::drag(),
                        );
                        if self.footprints.zoom == 0. {
                            self.footprints.zoom = 1.;
                            self.footprints.yaw = 0.6;
                            self.footprints.pitch = 0.7;
                        }
                        let d = ui.input(|i| i.pointer.delta());
                        if r.dragged_by(egui::PointerButton::Primary) {
                            self.footprints.yaw += d.x as f64 * 0.01;
                            self.footprints.pitch += d.y as f64 * 0.01;
                        }
                        if r.dragged_by(egui::PointerButton::Secondary) {
                            self.footprints.pan += d;
                        }
                        if r.hovered() {
                            self.footprints.zoom = (self.footprints.zoom
                                * (ui.input(|i| i.smooth_scroll_delta.y) * 0.002).exp()
                                * ui.input(|i| i.zoom_delta()))
                            .clamp(0.1, 30.);
                        }
                        pcb_ui::paint_mesh(
                            ui,
                            rect,
                            mesh,
                            self.footprints.yaw,
                            self.footprints.pitch,
                            self.footprints.zoom,
                            self.footprints.pan,
                            true,
                        );
                    }
                });
            self.footprints.preview_open = open;
        }

        if self.footprints.open {
            if !self.footprints.loaded {
                self.footprints.custom_only = true;
                self.footprints.sources = self.footprint_sources();
                self.footprints.loaded = true;
                self.footprints.epoch += 1;
            }
            let mut open = true;
            egui::Window::new("Footprint libraries · project and personal").open(&mut open).default_size(Vec2::new(1180.,650.)).show(ctx,|ui|{ui.horizontal(|ui|{if ui.button("Create footprint").clicked(){let def=pcb::footprint_source(&json!({"name":"NewFootprint","pads":[{"number":"1","kind":"smd","at":[0,0],"size":[1,1]}]})).unwrap();self.footprints.editor=Some(FootprintEditor{project_folder:None,native:def.document_text(),source:def,name:"NewFootprint".into(),library:"Custom".into(),scope:if self.doc.is_some()||self.pcb.board.is_some(){"project"}else{"personal"}.into(),revision:None,pad_number:"2".into(),model_path:String::new(),model_offset:[0.;3],model_scale:[1.;3],model_rotation:[0.;3],graphic:"Select".into(),first:None});}ui.checkbox(&mut self.footprints.custom_only,"Custom libraries only");ui.text_edit_singleline(&mut self.footprints.search);});ui.label("Expanded custom libraries · click any footprint to edit · installed edits create a personal copy.");ui.horizontal_top(|ui|{ui.vertical(|ui|{ui.set_width((ui.available_width()-320.).max(400.));egui::ScrollArea::vertical().max_height(520.).show(ui,|ui|{for(group,path)in self.footprints.sources.clone(){egui::CollapsingHeader::new(RichText::new(format!("{group} · {}",path.file_stem().unwrap_or_default().to_string_lossy())).strong().color(ACCENT)).default_open(true).id_salt((&path,self.footprints.epoch)).show(ui,|ui|self.footprint_cards(ui,&path));}if !self.footprints.custom_only{if let Some(libs)=&self.libraries{let ids:Vec<_>=libs.footprint_ids().into_iter().filter(|id|id.to_lowercase().contains(&self.footprints.search.to_lowercase())).take(120).collect();for id in ids{if ui.button(&id).clicked(){self.load_footprint_editor(&id,None);}}}ui.label("Search installed libraries to narrow the first 120 results.");}});});ui.vertical(|ui|{ui.set_width(290.);ui.heading("Footprint preview");if let Some(id)=self.footprints.selected.clone(){ui.label(&id);if let Some(libs)=&self.libraries&&let Ok(def)=libs.footprint(&id){let(rect,_)=ui.allocate_exact_size(Vec2::new(280.,230.),egui::Sense::hover());let scene=Scene::footprint_preview(&def);let mut view=View{show_grid:false,..Default::default()};view.fit_resource(&scene,rect);canvas::paint(&ui.painter_at(rect),&view,rect,&scene,None,false);ui.label(format!("{} pads · {} model links",def.children("pad").count(),def.children("model").count()));}if ui.button("Place on active PCB").clicked(){let reference=self.pcb.board.as_ref().map(|b|format!("U{}",b.root.children("footprint").count()+1)).unwrap_or("U1".into());self.pcb_job("pcb_edit",json!({"actions":[{"op":"add_footprint","library_id":id,"reference":reference,"at":self.cursor}]}),ctx);}}});});});
            self.footprints.open = open;
            if !open {
                self.footprints.loaded = false;
            }
        }
        let Some(mut editor) = self.footprints.editor.take() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Footprint editor · native KiCad library").open(&mut open).default_size(Vec2::new(1150.,700.)).show(ctx,|ui|{ui.horizontal_top(|ui|{ui.vertical(|ui|{ui.set_width(330.);egui::ScrollArea::vertical().max_height(600.).show(ui,|ui|{ui.heading("Identity and pads");ui.label("Footprint name");ui.text_edit_singleline(&mut editor.name);ui.label("Library");ui.text_edit_singleline(&mut editor.library);egui::ComboBox::from_label("Scope").selected_text(&editor.scope).show_ui(ui,|ui|{for scope in ["project","personal"]{ui.selectable_value(&mut editor.scope,scope.into(),scope);}});if ui.button("Save footprint").clicked(){let p=json!({"name":editor.name,"library":editor.library,"scope":editor.scope,"native_source":editor.source.document_text(),"expected_revision":editor.revision,"project_folder":editor.project_folder,"actor":"user"});match self.control_request("footprint_save",&p){Ok(v)=>{editor.revision=v["revision"].as_str().map(str::to_owned);editor.source.items_mut()[1]=Node::string(&editor.name);self.status=format!("Saved {}",v["id"]);self.footprints.sources=self.footprint_sources();},Err(e)=>self.fail(e)}}ui.horizontal(|ui|{if ui.add_enabled(editor.revision.is_some(),egui::Button::new("Rename saved footprint")).clicked(){let p=json!({"name":editor.source.value(1),"new_name":editor.name,"library":editor.library,"scope":editor.scope,"expected_revision":editor.revision,"project_folder":editor.project_folder,"action":"rename","actor":"user"});match self.control_request("footprint_manage",&p){Ok(v)=>{editor.revision=v["revision"].as_str().map(str::to_owned);editor.source.items_mut()[1]=Node::string(&editor.name);self.footprints.sources=self.footprint_sources();},Err(e)=>self.fail(e)}}if ui.add_enabled(editor.revision.is_some(),egui::Button::new("Delete (recoverable)")).clicked(){if rfd::MessageDialog::new().set_title("Delete custom footprint?").set_description("A recovery copy will be kept. Placed board instances are preserved.").set_buttons(rfd::MessageButtons::OkCancel).show()==rfd::MessageDialogResult::Ok{let p=json!({"name":editor.source.value(1),"library":editor.library,"scope":editor.scope,"expected_revision":editor.revision,"project_folder":editor.project_folder,"action":"delete","actor":"user"});match self.control_request("footprint_manage",&p){Ok(_)=>{editor.revision=None;self.footprints.sources=self.footprint_sources();self.status="Footprint deleted · recovery copy retained".into();},Err(e)=>self.fail(e)}}}});ui.separator();let mut remove=None;for(i,pad)in editor.source.items_mut().iter_mut().enumerate().filter(|(_,n)|n.name()=="pad"){egui::CollapsingHeader::new(format!("Pad {} · {} {}",pad.value(1),pad.value(2),pad.value(3))).id_salt(i).default_open(true).show(ui,|ui|{let mut number=pad.value(1).to_owned();if ui.text_edit_singleline(&mut number).changed(){pad.items_mut()[1]=Node::string(number);}let mut kind=pad.value(2).to_owned();egui::ComboBox::from_id_salt((i,"kind")).selected_text(&kind).show_ui(ui,|ui|{for k in ["smd","thru_hole","np_thru_hole"]{ui.selectable_value(&mut kind,k.into(),k);}});if pad.value(2)!=kind{pad.set_child(pcb::node("layers",if kind=="smd"{vec![Node::string("F.Cu"),Node::string("F.Paste"),Node::string("F.Mask")]}else{vec![Node::string("*.Cu"),Node::string("*.Mask")]}));if kind=="smd"{pad.items_mut().retain(|n|n.name()!="drill");}}pad.items_mut()[2]=Node::atom(&kind);let mut shape=pad.value(3).to_owned();egui::ComboBox::from_id_salt((i,"shape")).selected_text(&shape).show_ui(ui,|ui|{for s in ["rect","circle","oval","roundrect","custom"]{ui.selectable_value(&mut shape,s.into(),s);}});pad.items_mut()[3]=Node::atom(&shape);for key in ["at","size"]{let mut p=pcb::point(pad,key);ui.horizontal(|ui|{ui.label(format!("{key} mm"));ui.add(egui::DragValue::new(&mut p.0).speed(0.1));ui.add(egui::DragValue::new(&mut p.1).speed(0.1));});if key=="at"{let mut rotation=pcb::angle(pad);ui.horizontal(|ui|{ui.label("Rotation °");ui.add(egui::DragValue::new(&mut rotation));});pad.set_child(pcb::node("at",vec![Node::atom(p.0),Node::atom(p.1),Node::atom(rotation)]));}else{pad.set_child(pcb::xy(key,p));}}if kind!="smd"{let mut drill=pad.child("drill").map(|n|n.number(1)).unwrap_or(0.8);ui.horizontal(|ui|{ui.label("Drill mm");ui.add(egui::DragValue::new(&mut drill).speed(0.1));});pad.set_child(pcb::node("drill",vec![Node::atom(drill)]));}if ui.button("Remove pad").clicked(){remove=Some(i);}});}if let Some(i)=remove{editor.source.items_mut().remove(i);}ui.horizontal(|ui|{ui.text_edit_singleline(&mut editor.pad_number);if ui.button("Add pad").clicked(){if let Ok(def)=pcb::footprint_source(&json!({"name":"Pad","pads":[{"number":editor.pad_number,"kind":"smd","at":[0,0],"size":[1,1]}]})){editor.source.items_mut().push(def.children("pad").next().unwrap().clone());}}});ui.separator();ui.heading("3D models");let mut remove=None;for(i,model)in editor.source.items_mut().iter_mut().enumerate().filter(|(_,n)|n.name()=="model"){let mut path=model.value(1).to_owned();if ui.text_edit_singleline(&mut path).changed(){model.items_mut()[1]=Node::string(path);}for(key,default)in[("offset",[0.;3]),("rotate",[0.;3]),("scale",[1.;3])]{let mut v=default;if let Some(xyz)=model.child(key).and_then(|n|n.child("xyz")){for(j,n)in v.iter_mut().enumerate(){*n=xyz.number(j+1);}}ui.horizontal(|ui|{ui.label(key);for n in &mut v{ui.add(egui::DragValue::new(n).speed(0.1));}});model.set_child(pcb::node(key,vec![pcb::node("xyz",v.into_iter().map(Node::atom).collect())]));}if ui.button("Remove model").clicked(){remove=Some(i);}}if let Some(i)=remove{editor.source.items_mut().remove(i);}egui::CollapsingHeader::new("Create component body model").show(ui,|ui|{if self.footprints.body_size.iter().any(|v|*v<=0.){self.footprints.body_size=[5.,5.,3.];}ui.label("Measured package dimensions in mm; representative body geometry");ui.checkbox(&mut self.footprints.body_cylinder,"Cylindrical body");for (label,n) in ["Width / diameter","Depth","Height"].into_iter().zip(self.footprints.body_size.iter_mut()){ui.horizontal(|ui|{ui.label(label);ui.add(egui::DragValue::new(n).range(0.1..=1000.).speed(0.1));});}if ui.button("Create native VRML model").clicked(){let d=self.footprints.body_size;let shape=if self.footprints.body_cylinder{json!({"kind":"cylinder","radius_mm":d[0]/2.,"height_mm":d[2],"at_mm":[0,0,d[2]/2.],"color":[0.12,0.14,0.18]})}else{json!({"kind":"box","size_mm":d,"at_mm":[0,0,d[2]/2.],"color":[0.12,0.14,0.18]})};let name=format!("{}_body_{}",editor.name,uuid::Uuid::new_v4().simple());match self.control_request("model_create",&json!({"name":name,"shapes":[shape],"scope":editor.scope,"project_folder":editor.project_folder,"actor":"user"})){Ok(v)=>editor.model_path=v["reference"].as_str().unwrap_or("").into(),Err(e)=>self.fail(e)}}});ui.text_edit_singleline(&mut editor.model_path);if ui.button("Choose local STEP / VRML / mesh…").clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("3D models",&["step","stp","wrl","stl","obj","glb"]).pick_file(){editor.model_path=path.to_string_lossy().into_owned();}}if ui.button("Copy model into library scope").clicked(){match self.control_request("model_import",&json!({"path":editor.model_path,"scope":editor.scope,"project_folder":editor.project_folder,"actor":"user"})){Ok(v)=>editor.model_path=v["reference"].as_str().unwrap_or("").into(),Err(e)=>self.fail(e)}}if ui.button("Attach model").clicked(){let mut model=pcb::node("model",vec![Node::string(&editor.model_path)]);for(key,v)in[("offset",editor.model_offset),("scale",editor.model_scale),("rotate",editor.model_rotation)]{model.set_child(pcb::node(key,vec![pcb::node("xyz",v.into_iter().map(Node::atom).collect())]));}editor.source.items_mut().push(model);}if ui.button("Export native footprint…").clicked(){if let Some(path)=rfd::FileDialog::new().set_file_name(format!("{}.kicad_mod",editor.name)).save_file(){if let Err(e)=jenga_core::document::atomic_write(&path,editor.source.document_text().as_bytes(),true){self.fail(e);}}}});});ui.separator();ui.vertical(|ui|{ui.heading("Footprint preview");if ui.button("Preview footprint in 3D").clicked(){self.pcb_job("footprint_3d",json!({"name":editor.name,"native_source":editor.source.document_text(),"project_folder":editor.project_folder}),ctx);}ui.horizontal(|ui|{for tool in ["Select","Line","Rectangle","Circle"]{ui.selectable_value(&mut editor.graphic,tool.into(),tool);}ui.label("Click two points to draw");});let(rect,r)=ui.allocate_exact_size(Vec2::new(ui.available_width(),360.),egui::Sense::click());let scene=Scene::footprint_preview(&editor.source);let mut view=View{grid:0.5,..Default::default()};view.fit_resource(&scene,rect);canvas::paint(&ui.painter_at(rect),&view,rect,&scene,None,false);if r.clicked()&&editor.graphic!="Select"{if let Some(pos)=r.interact_pointer_pos(){let at=view.snap(view.world(pos,rect));if let Some(first)=editor.first.take(){let kind=match editor.graphic.as_str(){"Line"=>"fp_line","Rectangle"=>"fp_rect",_=>"fp_circle"};editor.source.items_mut().push(pcb::node(kind,vec![pcb::xy(if kind=="fp_circle"{"center"}else{"start"},first),pcb::xy("end",at),pcb::node("stroke",vec![pcb::node("width",vec![Node::atom(0.12)]),pcb::node("type",vec![Node::atom("default")])]),pcb::node("layer",vec![Node::string("F.SilkS")]),pcb::uid()]));}else{editor.first=Some(at);}}}egui::CollapsingHeader::new("Edit existing native footprint source").default_open(false).show(ui,|ui|{ui.label("Apply source explicitly; graphical edits preserve unsupported native fields.");ui.add(egui::TextEdit::multiline(&mut editor.native).code_editor().desired_rows(8).desired_width(ui.available_width()));if ui.button("Apply source").clicked(){match parse(&editor.native){Ok(n)if n.name()=="footprint"=>editor.source=n,Ok(_)=>self.fail("Expected native footprint"),Err(e)=>self.fail(e)}}if ui.button("Refresh source from preview").clicked(){editor.native=editor.source.document_text();}});});});});
        if open {
            self.footprints.editor = Some(editor);
        }
    }
    pub(super) fn archived_dialog(&mut self, ctx: &egui::Context) {
        if !self.archived_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Archived projects · restore")
            .open(&mut open)
            .default_size(Vec2::new(750., 450.))
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading("Archived projects");
                let archives = jenga_tools::projects::archived_projects();
                if let Some(entries) = archives["projects"].as_array() {
                    if entries.is_empty() {
                        ui.label("No archived projects.");
                    }
                    for entry in entries {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            let path = entry["path"].as_str().unwrap_or("");
                            ui.strong(
                                Path::new(path)
                                    .file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                            );
                            ui.label(path);
                            if ui
                                .add_enabled(
                                    entry["available"] == true,
                                    egui::Button::new("Restore to Home"),
                                )
                                .clicked()
                            {
                                let p = if entry["kind"] == "hidden" {
                                    json!({"path":path})
                                } else {
                                    json!({"restore_manifest":entry["restore_manifest"]})
                                };
                                match jenga_tools::projects::restore_project(&p) {
                                    Ok(v) => {
                                        if let Some(path) = v["file"].as_str() {
                                            self.settings.remember(PathBuf::from(path));
                                            self.persist();
                                        }
                                        self.status = "Project restored".into();
                                    }
                                    Err(e) => self.fail(e),
                                }
                            }
                        });
                    }
                }
            });
        self.archived_open = open;
    }
}
