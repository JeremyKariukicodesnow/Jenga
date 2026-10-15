use super::*;
use serde_json::json;
pub(super) struct SymbolEditor {
    name: String,
    library: String,
    prefix: String,
    description: String,
    footprint: String,
    personal: bool,
    width: f64,
    height: f64,
    pins: Vec<Pin>,
    graphics: String,
    load_id: String,
    native_source: String,
    value: String,
    shapes: Vec<serde_json::Value>,
    draw_tool: String,
    draft: Vec<Point>,
    draw_text: String,
    draw_fill: String,
    draw_stroke: f64,
    draw_body: bool,
    canvas_view: View,
    fit_canvas: bool,
    preview_unit: u32,
    replace_pins: bool,
    selected_shape: Option<usize>,
    last_drag: Option<Point>,
}
struct Pin {
    number: String,
    name: String,
    kind: String,
    x: f64,
    y: f64,
    angle: i64,
    unit: u64,
    shape: String,
    length: f64,
    hidden: bool,
}
impl Default for SymbolEditor {
    fn default() -> Self {
        Self {
            name: "MySymbol".into(),
            library: "MySymbols".into(),
            prefix: "U".into(),
            description: String::new(),
            footprint: String::new(),
            personal: true,
            width: 15.24,
            height: 20.32,
            pins: vec![
                Pin {
                    number: "1".into(),
                    name: "IN".into(),
                    kind: "input".into(),
                    x: -12.7,
                    y: 5.08,
                    angle: 0,
                    unit: 1,
                    shape: "line".into(),
                    length: 5.08,
                    hidden: false,
                },
                Pin {
                    number: "2".into(),
                    name: "OUT".into(),
                    kind: "output".into(),
                    x: 12.7,
                    y: 5.08,
                    angle: 180,
                    unit: 1,
                    shape: "line".into(),
                    length: 5.08,
                    hidden: false,
                },
            ],
            graphics: String::new(),
            load_id: String::new(),
            native_source: String::new(),
            value: String::new(),
            shapes: vec![],
            draw_tool: "Select".into(),
            draft: vec![],
            draw_text: "Label".into(),
            draw_fill: "none".into(),
            draw_stroke: 0.254,
            draw_body: true,
            canvas_view: View::default(),
            fit_canvas: true,
            preview_unit: 1,
            replace_pins: true,
            selected_shape: None,
            last_drag: None,
        }
    }
}
impl SymbolEditor {
    fn params(&self) -> serde_json::Value {
        let mut p = json!({"name":self.name,"library":self.library,"prefix":self.prefix,"description":self.description,"footprint":self.footprint,"scope":if self.personal{"personal"}else{"project"},"width_mm":self.width,"height_mm":self.height,"graphics":self.graphics,"pins":self.pins.iter().map(|p|json!({"number":p.number,"name":p.name,"kind":p.kind,"at":[p.x,p.y],"angle":p.angle,"unit":p.unit,"shape":p.shape,"length_mm":p.length,"hidden":p.hidden})).collect::<Vec<_>>(),"license":"CC0-1.0","attribution":"User-authored in Jenga symbol editor","actor":"user"});
        p["shapes"] = json!(self.shapes);
        p["draw_body"] = json!(self.draw_body);
        p["replace_pins"] = json!(self.replace_pins);
        p["value"] = json!(if self.value.is_empty() {
            &self.name
        } else {
            &self.value
        });
        if !self.native_source.is_empty() {
            p["native_source"] = json!(self.native_source);
            p["license"] = json!("Original source library license applies; not relicensed");
            p["attribution"] = json!(format!("Derived from {}", self.load_id));
        }
        p
    }
}
impl App {
    pub(super) fn symbol_editor_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.symbol_editor.take() else {
            return;
        };
        let mut open = true;
        let mut saved = false;
        egui::Window::new("Symbol Editor • native KiCad library").open(&mut open).default_size([980.,680.]).show(ctx,|ui|{
            ui.horizontal(|ui|{
                ui.vertical(|ui|{
                    ui.set_width(270.);ui.heading("Identity and library");
                    ui.label("Load existing symbol (Library:Name)");ui.text_edit_singleline(&mut editor.load_id);
                    if ui.button("Load native symbol").clicked(){
                        if let Some(libs)=self.libraries.as_mut(){match libs.symbol(&editor.load_id){Ok(def)=>{editor.name=def.value(1).into();editor.footprint=property(&def,"Footprint").into();editor.description=property(&def,"Description").into();editor.prefix=property(&def,"Reference").into();editor.value=property(&def,"Value").into();editor.pins.clear();for unit in def.children("symbol"){let number=unit.value(1).rsplit('_').nth(1).and_then(|v|v.parse::<u64>().ok()).unwrap_or(1);for pin in unit.children("pin"){let(x,y,angle)=position(pin);editor.pins.push(Pin{number:pin.child("number").map(|n|n.value(1)).unwrap_or("").into(),name:pin.child("name").map(|n|n.value(1)).unwrap_or("").into(),kind:pin.value(1).into(),x,y,angle:angle as i64,unit:number,shape:pin.value(2).into(),length:pin.child("length").map(|n|n.number(1)).unwrap_or(5.08),hidden:pin.items().iter().any(|n|n.text()=="hide")});}}editor.native_source=def.encoded();editor.fit_canvas=true;editor.shapes.clear();editor.draft.clear();},Err(e)=>self.fail(e)}}else{self.status="Wait for library indexing".into();}
                    }
                    if !editor.native_source.is_empty(){ui.label("Existing graphics and all units are preserved. Edit native source below, or create a new symbol.");if ui.button("Create new structured symbol").clicked(){editor.native_source.clear();}}

                    for(label,value)in[("Symbol name",&mut editor.name),("Library",&mut editor.library),("Reference prefix",&mut editor.prefix),("Value label",&mut editor.value),("Description",&mut editor.description),("Default footprint ID",&mut editor.footprint)]{ui.label(label);ui.text_edit_singleline(value);}
                    ui.checkbox(&mut editor.draw_body,"Default rectangular body");
                    ui.checkbox(&mut editor.personal,"Personal library • available to all projects");
                    ui.label("Project libraries use ${KIPRJMOD}; personal libraries persist outside the installation.");
                    ui.horizontal(|ui|{ui.label("Body mm");ui.add(egui::DragValue::new(&mut editor.width).range(2.54..=500.).speed(1.27));ui.add(egui::DragValue::new(&mut editor.height).range(2.54..=500.).speed(1.27));});
                    ui.label("Footprint IDs are checked before saving. A footprint can carry native STEP/VRML model links.");
                    if ui.button("Browse existing footprints").clicked(){self.library_dialog=true;}
                    egui::CollapsingHeader::new("Native graphics (advanced)").show(ui,|ui|{ui.label("KiCad rectangle, polyline, circle, arc and text primitives");ui.add(egui::TextEdit::multiline(&mut editor.graphics).code_editor().desired_rows(7));});
                    if crate::design::button(ui,"Save","save",false).clicked(){
                        let result=(||->anyhow::Result<serde_json::Value>{let mut p=editor.params();let base=if editor.personal{jenga_tools::symbol_editor::personal_folder()?}else{self.doc.as_ref().and_then(|d|d.path.as_ref()).and_then(|p|p.parent()).ok_or_else(||anyhow::anyhow!("Save a project first"))?.to_owned()};let path=base.join("libraries").join(format!("{}.kicad_sym",editor.library));
                            if path.is_file(){let root=jenga_core::sexpr::parse(&std::fs::read_to_string(&path)?)?;if root.children("symbol").any(|s|s.value(1)==editor.name)&&rfd::MessageDialog::new().set_title("Update existing symbol?").set_description("Replace this symbol definition in the selected library? Existing schematic instances keep their embedded definition until refreshed.").set_buttons(rfd::MessageButtons::OkCancel).show()!=rfd::MessageDialogResult::Ok{return Err(anyhow::anyhow!("Symbol save cancelled"));}p["expected_revision"]=json!(jenga_tools::journal::revision_text(&root.encoded()));}
                            self.control_request("save_symbol",&p)
                        })();match result{Ok(_)=>{self.status=format!("Saved {}:{} to native symbol library",editor.library,editor.name);self.reload_libraries();saved=true;},Err(e)=>self.fail(e)}
                    }
                });
                ui.separator();ui.vertical(|ui|{
                    ui.heading("Symbol preview");
                    if !editor.native_source.is_empty(){egui::CollapsingHeader::new("Edit existing native symbol source").default_open(true).show(ui,|ui|{ui.add(egui::TextEdit::multiline(&mut editor.native_source).code_editor().desired_rows(8));});}

                    editor.drawing_toolbar(ui);
                    let params=editor.params();
                    match jenga_tools::symbol_editor::symbol(&params){Ok(def)=>{let scene=Scene::symbol_preview_unit(&def,editor.preview_unit);editor.drawing_canvas(ui,&scene);},Err(e)=>{ui.colored_label(ACCENT,e.to_string());}}
                    ui.label("Pins: library coordinates in millimetres; electrical endpoints should use a 1.27mm grid.");
                    let mut delete=None;
                    egui::ScrollArea::both().max_height(280.).show(ui,|ui|{egui::Grid::new("symbol_editor_pins").striped(true).show(ui,|ui|{
                        for h in ["Number","Name / label","Electrical type","Graphic","X","Y","Angle","Unit","Length","Hidden",""]{ui.label(h);}ui.end_row();
                        for(i,p)in editor.pins.iter_mut().enumerate(){ui.add(egui::TextEdit::singleline(&mut p.number).desired_width(38.));ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(80.));egui::ComboBox::from_id_salt(("pin_type",i)).selected_text(&p.kind).show_ui(ui,|ui|{for kind in ["passive","input","output","bidirectional","tri_state","power_in","power_out","open_collector","open_emitter","free","unspecified","no_connect"]{ui.selectable_value(&mut p.kind,kind.into(),kind);}});egui::ComboBox::from_id_salt(("pin_graphic",i)).selected_text(&p.shape).show_ui(ui,|ui|{for shape in ["line","inverted","clock","inverted_clock","input_low","clock_low","output_low","edge_clock_high","non_logic"]{ui.selectable_value(&mut p.shape,shape.into(),shape);}});ui.add(egui::DragValue::new(&mut p.x).speed(1.27));ui.add(egui::DragValue::new(&mut p.y).speed(1.27));ui.add(egui::DragValue::new(&mut p.angle).speed(90));ui.add(egui::DragValue::new(&mut p.unit).range(0..=100));ui.add(egui::DragValue::new(&mut p.length).range(0.0..=100.).speed(1.27));ui.checkbox(&mut p.hidden,"");if ui.small_button("×").on_hover_text("Remove pin").clicked(){delete=Some(i);}ui.end_row();}
                    });});if let Some(i)=delete{editor.pins.remove(i);}
                    if ui.button("+ Add pin").clicked(){editor.pins.push(Pin{number:(editor.pins.len()+1).to_string(),name:String::new(),kind:"passive".into(),x:-12.7,y:-(editor.pins.len() as f64)*2.54,angle:0,unit:1,shape:"line".into(),length:5.08,hidden:false});}
                });
            });
        });
        if open && !saved {
            self.symbol_editor = Some(editor);
        }
    }
}
impl SymbolEditor {
    fn drawing_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for tool in [
                "Select",
                "Line",
                "Polyline",
                "Polygon",
                "Rectangle",
                "Circle",
                "Arc",
                "Text",
                "Pin",
            ] {
                if ui.selectable_label(self.draw_tool == tool, tool).clicked() {
                    self.draw_tool = tool.into();
                    self.draft.clear();
                }
            }
            if ui.button("Fit").clicked() {
                self.fit_canvas = true;
            }
            if ui.button("Undo shape").clicked() {
                self.shapes.pop();
                self.selected_shape = None;
                self.draft.clear();
            }
            if ui.button("Cancel drawing").clicked() {
                self.draft.clear();
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Stroke");
            ui.add(
                egui::DragValue::new(&mut self.draw_stroke)
                    .range(0.05..=5.)
                    .speed(0.05)
                    .suffix("mm"),
            );
            egui::ComboBox::from_id_salt("symbol_fill")
                .selected_text(&self.draw_fill)
                .show_ui(ui, |ui| {
                    for fill in ["none", "background", "outline"] {
                        ui.selectable_value(&mut self.draw_fill, fill.into(), fill);
                    }
                });
            ui.label("Preview unit");
            ui.add(egui::DragValue::new(&mut self.preview_unit).range(1..=100));
            if self.draw_tool == "Text" {
                ui.add(
                    egui::TextEdit::singleline(&mut self.draw_text)
                        .desired_width(130.)
                        .hint_text("Symbol label"),
                );
            }
            if ["Polyline", "Polygon"].contains(&self.draw_tool.as_str())
                && ui.button("Finish path").clicked()
            {
                self.finish_path();
            }
            if !self.native_source.is_empty() {
                ui.checkbox(
                    &mut self.replace_pins,
                    "Apply pin-table changes to loaded symbol",
                );
            }
        });
        ui.label("Click points to draw. Arc: start, point on arc, end. Select and drag added shapes to reposition. Pin numbers are physical terminals; unit 0 is common.");
    }
    fn finish_path(&mut self) {
        if self.draft.len() >= 2 {
            self.shapes.push(json!({"kind":if self.draw_tool=="Polygon"{"polygon"}else{"polyline"},"points":self.draft,"stroke_mm":self.draw_stroke,"fill":self.draw_fill}));
            self.draft.clear();
        }
    }
    fn drawing_canvas(&mut self, ui: &mut egui::Ui, scene: &Scene) {
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().max(300.), 230.),
            egui::Sense::click_and_drag(),
        );
        if self.fit_canvas {
            self.canvas_view.fit(scene, rect);
            self.canvas_view.show_grid = true;
            self.fit_canvas = false;
        }
        if response.hovered() {
            let delta = ui.input(|i| i.smooth_scroll_delta.y);
            if delta != 0. {
                self.canvas_view.scale =
                    (self.canvas_view.scale * (delta * 0.005).exp()).clamp(0.5, 60.);
            }
        }
        canvas::paint(
            &ui.painter_at(rect),
            &self.canvas_view,
            rect,
            scene,
            None,
            false,
        );
        let point = response.interact_pointer_pos().map(|pos| {
            let p = self.canvas_view.snap(self.canvas_view.world(pos, rect));
            (p.0, -p.1)
        });
        if self.draw_tool == "Select" {
            if response.drag_started() {
                self.last_drag = point;
                if let Some(p) = point {
                    self.selected_shape =
                        self.shapes.iter().enumerate().rev().find_map(|(i, s)| {
                            let (lo, hi) = graphic_bounds(s);
                            (p.0 >= lo.0 - 1.27
                                && p.0 <= hi.0 + 1.27
                                && p.1 >= lo.1 - 1.27
                                && p.1 <= hi.1 + 1.27)
                                .then_some(i)
                        });
                }
            }
            if response.dragged()
                && let (Some(p), Some(old), Some(i)) = (point, self.last_drag, self.selected_shape)
            {
                shift_graphic(&mut self.shapes[i], p.0 - old.0, p.1 - old.1);
                self.last_drag = Some(p);
            }
            if response.drag_stopped() {
                self.last_drag = None;
            }
        } else if response.clicked()
            && let Some(p) = point
        {
            if self.draw_tool == "Text" {
                self.shapes
                    .push(json!({"kind":"text","at":p,"text":self.draw_text,"size_mm":1.27}));
            } else if self.draw_tool == "Pin" {
                let mut number = 1;
                while self.pins.iter().any(|p| p.number == number.to_string()) {
                    number += 1;
                }
                self.pins.push(Pin {
                    number: number.to_string(),
                    name: format!("PIN{number}"),
                    kind: "passive".into(),
                    x: p.0,
                    y: p.1,
                    angle: if p.0 > 0. { 180 } else { 0 },
                    unit: self.preview_unit as u64,
                    shape: "line".into(),
                    length: 5.08,
                    hidden: false,
                });
            } else {
                self.draft.push(p);
                let need = if self.draw_tool == "Arc" { 3 } else { 2 };
                if !["Polyline", "Polygon"].contains(&self.draw_tool.as_str())
                    && self.draft.len() == need
                {
                    let a = self.draft[0];
                    let b = self.draft[1];
                    let shape = match self.draw_tool.as_str() {
                        "Rectangle" => json!({"kind":"rectangle","start":a,"end":b}),
                        "Circle" => {
                            json!({"kind":"circle","center":a,"radius_mm":(a.0-b.0).hypot(a.1-b.1)})
                        }
                        "Arc" => json!({"kind":"arc","start":a,"mid":b,"end":self.draft[2]}),
                        _ => json!({"kind":"line","points":[a,b]}),
                    };
                    let mut shape = shape;
                    shape["stroke_mm"] = json!(self.draw_stroke);
                    shape["fill"] = json!(self.draw_fill);
                    self.shapes.push(shape);
                    self.draft.clear();
                }
            }
        }
        if response.double_clicked() && ["Polyline", "Polygon"].contains(&self.draw_tool.as_str()) {
            self.finish_path();
        }
        let painter = ui.painter_at(rect);
        for p in &self.draft {
            painter.circle_filled(self.canvas_view.screen((p.0, -p.1), rect), 3., ACCENT);
        }
        if let (Some(a), Some(b)) = (self.draft.last(), point) {
            painter.line_segment(
                [
                    self.canvas_view.screen((a.0, -a.1), rect),
                    self.canvas_view.screen((b.0, -b.1), rect),
                ],
                Stroke::new(1_f32, ACCENT),
            );
        }
        if let Some(i) = self.selected_shape.filter(|i| *i < self.shapes.len()) {
            let (lo, hi) = graphic_bounds(&self.shapes[i]);
            let r = egui::Rect::from_two_pos(
                self.canvas_view.screen((lo.0, -lo.1), rect),
                self.canvas_view.screen((hi.0, -hi.1), rect),
            );
            painter.rect_stroke(
                r.expand(3.),
                0.,
                Stroke::new(1_f32, ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        egui::CollapsingHeader::new(format!("Added shapes ({})", self.shapes.len())).show(
            ui,
            |ui| {
                let mut remove = None;
                for (i, s) in self.shapes.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                self.selected_shape == Some(i),
                                format!("{} {}", i + 1, s["kind"].as_str().unwrap_or("shape")),
                            )
                            .clicked()
                        {
                            self.selected_shape = Some(i);
                        }
                        if s["kind"] == "text" {
                            let mut text = s["text"].as_str().unwrap_or("").to_owned();
                            if ui.text_edit_singleline(&mut text).changed() {
                                s["text"] = json!(text);
                            }
                        }
                        if ui.small_button("×").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    self.shapes.remove(i);
                    self.selected_shape = None;
                }
            },
        );
    }
}
fn graphic_bounds(s: &serde_json::Value) -> (Point, Point) {
    let mut pts = vec![];
    for key in ["start", "end", "mid", "at", "center"] {
        if let Some(p) = s[key].as_array() {
            if p.len() == 2 {
                pts.push((p[0].as_f64().unwrap_or(0.), p[1].as_f64().unwrap_or(0.)));
            }
        }
    }
    for p in s["points"].as_array().into_iter().flatten() {
        if let Some(p) = p.as_array() {
            if p.len() == 2 {
                pts.push((p[0].as_f64().unwrap_or(0.), p[1].as_f64().unwrap_or(0.)));
            }
        }
    }
    if s["kind"] == "circle"
        && let Some(center) = pts.first().copied()
    {
        let r = s["radius_mm"].as_f64().unwrap_or(0.);
        pts = vec![(center.0 - r, center.1 - r), (center.0 + r, center.1 + r)];
    }
    let mut lo = (f64::INFINITY, f64::INFINITY);
    let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in pts {
        lo = (lo.0.min(p.0), lo.1.min(p.1));
        hi = (hi.0.max(p.0), hi.1.max(p.1));
    }
    (lo, hi)
}
fn shift_graphic(s: &mut serde_json::Value, dx: f64, dy: f64) {
    for key in ["start", "end", "mid", "at", "center"] {
        if let Some(p) = s[key].as_array_mut() {
            if p.len() == 2 {
                p[0] = json!(p[0].as_f64().unwrap_or(0.) + dx);
                p[1] = json!(p[1].as_f64().unwrap_or(0.) + dy);
            }
        }
    }
    if let Some(ps) = s["points"].as_array_mut() {
        for p in ps {
            if let Some(p) = p.as_array_mut() {
                if p.len() == 2 {
                    p[0] = json!(p[0].as_f64().unwrap_or(0.) + dx);
                    p[1] = json!(p[1].as_f64().unwrap_or(0.) + dy);
                }
            }
        }
    }
}
