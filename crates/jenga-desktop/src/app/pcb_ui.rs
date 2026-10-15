//! Native PCB canvas and background engineering jobs shared with external tools.
use super::*;
use anyhow::{Context, Result, ensure};
use jenga_core::{
    mesh::Mesh,
    pcb::{self, Board},
};
use serde_json::{Value, json};
pub(super) struct Completed {
    engine: jenga_tools::engine::Engine,
    result: Result<Value>,
    base_board: Option<String>,
    base_doc: Option<String>,
    method: String,
    params: Value,
    id: String,
    mesh: Option<Mesh>,
}
pub(super) struct PcbUi {
    pub board: Option<Board>,
    pub active: bool,
    pub pending: Option<mpsc::Receiver<Completed>>,
    pub fit: bool,
    scene_cache: Option<(String, Scene)>,
    view: View,
    selected: Option<usize>,
    layer: String,
    visible_layers: Option<Vec<String>>,
    show_ratsnest: bool,
    tool: String,
    start: Option<Value>,
    points: Vec<Point>,
    width: f64,
    reference: String,
    value: String,
    library_id: String,
    text: String,
    net: String,
    export_format: String,
    pub result: Option<Value>,
    pub preview_dialog: bool,
    preview_texture: Option<egui::TextureHandle>,
    pub show_3d: bool,
    mesh: Option<Mesh>,
    yaw: f64,
    pitch: f64,
    zoom: f32,
    pan: Vec2,
    solid: bool,
}
impl Default for PcbUi {
    fn default() -> Self {
        Self {
            board: None,
            active: false,
            pending: None,
            fit: true,
            scene_cache: None,
            view: View {
                grid: 0.5,
                ..Default::default()
            },
            selected: None,
            layer: "F.Cu".into(),
            visible_layers: None,
            show_ratsnest: true,
            tool: "Select".into(),
            start: None,
            points: vec![],
            width: 0.25,
            reference: "U1".into(),
            value: String::new(),
            library_id: String::new(),
            text: "Text".into(),
            net: String::new(),
            export_format: "manufacturing".into(),
            result: None,
            preview_dialog: false,
            preview_texture: None,
            show_3d: false,
            mesh: None,
            yaw: 0.6,
            pitch: 0.7,
            zoom: 1.,
            pan: Vec2::ZERO,
            solid: true,
        }
    }
}
impl App {
    pub(super) fn start_pcb_worker(
        &mut self,
        method: &str,
        p: &Value,
        ctx: &egui::Context,
    ) -> Result<Value> {
        ensure!(
            self.pcb.pending.is_none(),
            "A PCB operation is running; read its operation_status before starting another"
        );
        let mut p = p.clone();
        let id = p["operation_id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        p["operation_id"] = json!(id);
        if let Some(receipt) = jenga_tools::operations::begin(&id, method, &p)? {
            return jenga_tools::engine::Engine::operation_response(receipt);
        }
        let workspace = self
            .pcb
            .board
            .as_ref()
            .and_then(|b| b.path.as_ref())
            .or_else(|| self.doc.as_ref()?.path.as_ref())
            .and_then(|p| p.parent())
            .map(Path::to_owned)
            .unwrap_or(std::env::current_dir()?);
        let mut engine = jenga_tools::engine::Engine::new(workspace);
        engine.doc = self.doc.clone();
        engine.board = self.pcb.board.clone();
        engine.active_kind = if self.pcb.active { "pcb" } else { "schematic" }.into();
        let base_board = engine.board.as_ref().map(Board::revision);
        let base_doc = engine.doc.as_ref().map(jenga_tools::journal::revision);
        let (tx, rx) = mpsc::channel();
        let method = method.to_owned();
        let wake = ctx.clone();
        let operation = id.clone();
        self.status = format!("PCB · {method} running");
        std::thread::spawn(move || {
            let mut result = engine.execute_staged(&method, &p);
            let mesh = if method == "pcb_3d" || method == "footprint_3d" {
                if let Ok(v) = &result {
                    if let Some(path) = v["path"].as_str() {
                        match Mesh::read(Path::new(path)) {
                            Ok(m) => Some(m),
                            Err(e) => {
                                result = Err(e);
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            let _ = tx.send(Completed {
                engine,
                result,
                base_board,
                base_doc,
                method,
                params: p,
                id: operation,
                mesh,
            });
            wake.request_repaint();
        });
        self.pcb.pending = Some(rx);
        Ok(
            json!({"operation_id":id,"status":"pending","message":"Accepted; query operation_status with this ID. Do not repeat the edit."}),
        )
    }
    pub(super) fn poll_pcb_worker(&mut self, ctx: &egui::Context) {
        let completed = self.pcb.pending.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(mut job) = completed {
            self.pcb.pending = None;
            if job.result.is_ok()
                && (job.base_board != self.pcb.board.as_ref().map(Board::revision)
                    || job.base_doc != self.doc.as_ref().map(jenga_tools::journal::revision))
            {
                job.result = Err(anyhow::anyhow!(
                    "Project changed while operation was running; candidate was not applied"
                ));
            }
            if job.result.is_ok() {
                let actor = job.params["actor"].as_str().unwrap_or("external");
                let changed = job.engine.board.as_ref().map(Board::revision) != job.base_board;
                if changed {
                    if let Some(b) = &job.engine.board {
                        if let Err(e) = jenga_tools::pcb_tools::record(
                            self.pcb.board.as_ref(),
                            b,
                            actor,
                            &job.method,
                        ) {
                            job.result = Err(e);
                        }
                    }
                }
                if job.result.is_ok()
                    && job.engine.doc.as_ref().map(jenga_tools::journal::revision) != job.base_doc
                {
                    if let (Some(a), Some(b)) = (&self.doc, &job.engine.doc) {
                        if let Err(e) = jenga_tools::journal::record(a, b, actor, &job.method) {
                            job.result = Err(e);
                        }
                    }
                }
                if job.result.is_ok() {
                    let selected_uuid = self
                        .pcb
                        .selected
                        .and_then(|i| self.pcb.board.as_ref()?.root.items().get(i))
                        .map(|n| pcb::id(n).to_owned());
                    self.pcb.board = job.engine.board;
                    self.pcb.selected = selected_uuid.and_then(|uuid| {
                        self.pcb
                            .board
                            .as_ref()?
                            .root
                            .items()
                            .iter()
                            .position(|n| pcb::id(n) == uuid)
                    });
                    self.doc = job.engine.doc;
                    self.observed_doc = self.doc.clone();
                    if matches!(
                        job.method.as_str(),
                        "footprint_save" | "footprint_manage" | "footprint_copy"
                    ) {
                        self.reload_libraries();
                        self.refresh_footprint_sources();
                    }
                    if self.pcb.board.is_some() {
                        self.pcb.active = job.engine.active_kind == "pcb";
                        if changed
                            || matches!(
                                job.method.as_str(),
                                "pcb_open" | "pcb_new" | "pcb_create_project"
                            )
                        {
                            self.workspace_ui.show = false;
                        }
                    }
                    if changed {
                        self.pcb.fit = true;
                        self.pcb.mesh = None;
                    }
                    if let Some(path) = self.pcb.board.as_ref().and_then(|b| b.path.clone()) {
                        self.workspace_ui.root = Some(path.clone());
                        self.settings.remember(path);
                        self.persist();
                    }
                    if job.method == "footprint_3d" {
                        self.footprints.preview_mesh = job.mesh.take();
                        self.footprints.preview_open = true;
                    }
                    if job.method == "pcb_3d" {
                        self.pcb.mesh = job.mesh.take();
                        self.pcb.show_3d = true;
                    }
                }
            }
            let _ = jenga_tools::operations::finish(&job.id, &job.result);
            match job.result {
                Ok(mut v) => {
                    v["operation_id"] = json!(job.id);
                    v["status"] = json!("completed");
                    self.status = format!("PCB · {} completed", job.method);
                    if v["preview_id"].is_string() {
                        self.pcb.preview_dialog = true;
                        if let Some(path) = v["preview_image_path"].as_str() {
                            if let Ok(im) = image::open(path) {
                                let im = im.into_rgba8();
                                let color = egui::ColorImage::from_rgba_unmultiplied(
                                    [im.width() as usize, im.height() as usize],
                                    im.as_raw(),
                                );
                                self.pcb.preview_texture = Some(ctx.load_texture(
                                    "pcb_batch_preview",
                                    color,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                        }
                    } else if job.method == "pcb_commit" {
                        self.pcb.preview_dialog = false;
                    }
                    self.pcb.result = Some(v);
                }
                Err(e) => self.fail(format!("{}: {e:#}", job.method)),
            }
            self.publish_control_snapshot();
        } else if self.pcb.pending.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    pub(super) fn pcb_job(&mut self, method: &str, p: Value, ctx: &egui::Context) {
        let mut p = p;
        p["actor"] = json!("user");
        if let Err(e) = self.start_pcb_worker(method, &p, ctx) {
            self.fail(e);
        }
    }
    pub(super) fn save_pcb(&mut self) -> bool {
        if self.pcb.pending.is_some() {
            self.status = "Wait for the PCB operation to finish before saving".into();
            return false;
        }
        let path = self
            .pcb
            .board
            .as_ref()
            .and_then(|b| b.path.clone())
            .or_else(|| {
                rfd::FileDialog::new()
                    .add_filter("KiCad PCB", &["kicad_pcb"])
                    .set_file_name("Board.kicad_pcb")
                    .save_file()
            });
        let Some(path) = path else {
            return false;
        };
        match self
            .pcb
            .board
            .as_mut()
            .context("No PCB")
            .and_then(|b| b.save(&path))
        {
            Ok(()) => {
                self.settings.remember(path);
                self.persist();
                self.status = "PCB saved".into();
                true
            }
            Err(e) => {
                self.fail(e);
                false
            }
        }
    }
    pub(super) fn pcb_panels(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("pcb_tools").show_inside(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("⌂ Project").clicked() {
                    self.workspace_ui.show = true;
                    self.workspace_ui.reset_scroll = true;
                }
                if ui.button("Save").clicked() {
                    self.save_pcb();
                }
                if self.doc.is_some() && ui.button("Schematic").clicked() {
                    self.pcb.active = false;
                }
                for tool in [
                    "Select", "Move", "Route", "Track", "Via", "Outline", "Zone", "Keepout", "Text",
                ] {
                    if ui.selectable_label(self.pcb.tool == tool, tool).clicked() {
                        self.pcb.tool = tool.into();
                        self.pcb.start = None;
                        self.pcb.points.clear();
                    }
                }
                if ui.button("Fit").clicked() {
                    self.pcb.fit = true;
                }
                if ui.button("Footprints").clicked() {
                    self.footprints.open = true;
                }
                ui.add_enabled_ui(self.pcb.pending.is_none(), |ui| {
                    for (label, method, p) in [
                        ("Undo", "pcb_undo", json!({})),
                        ("Redo", "pcb_redo", json!({})),
                        (
                            "Preview placement",
                            "pcb_autoplace",
                            json!({"dry_run":true}),
                        ),
                        ("Preview routing", "pcb_autoroute", json!({"dry_run":true})),
                        ("Sync schematic", "pcb_sync", json!({"dry_run":true})),
                        ("Fill zones", "pcb_fill_zones", json!({})),
                        ("Verify", "pcb_verify", json!({"external":true})),
                        ("3D", "pcb_3d", json!({})),
                    ] {
                        if ui.button(label).clicked() {
                            self.pcb_job(method, p, &ctx);
                        }
                    }
                });
            });
            if self.pcb.pending.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        "Engineering operation running · the canvas remains available to inspect",
                    );
                });
            }
        });
        egui::Panel::right("pcb_inspector")
            .default_size(270.)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.pcb_properties(ui));
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| self.pcb_canvas(ui));
    }
    fn pcb_properties(&mut self, ui: &mut egui::Ui) {
        let Some(b) = self.pcb.board.as_ref().map(Board::snapshot) else {
            return;
        };
        let ctx = ui.ctx().clone();
        ui.heading("Board & selection");
        ui.label(format!(
            "{} footprints · {} tracks",
            b.root.children("footprint").count(),
            b.root.children("segment").count()
        ));
        ui.label(if b.schematic.is_some() {
            "Linked schematic"
        } else {
            "Independent PCB"
        });
        if let Some(s) = &b.schematic {
            ui.label(s.display().to_string());
        }
        egui::ComboBox::from_label("Active copper layer")
            .selected_text(&self.pcb.layer)
            .show_ui(ui, |ui| {
                for l in b.copper_layers() {
                    ui.selectable_value(&mut self.pcb.layer, l.clone(), l);
                }
            });
        ui.horizontal(|ui| {
            ui.label("Track mm");
            ui.add(
                egui::DragValue::new(&mut self.pcb.width)
                    .range(0.05..=20.)
                    .speed(0.05),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Grid mm");
            ui.add(
                egui::DragValue::new(&mut self.pcb.view.grid)
                    .range(0.05..=10.)
                    .speed(0.05),
            );
        });
        ui.checkbox(&mut self.pcb.view.show_grid, "Grid");
        ui.checkbox(&mut self.pcb.show_ratsnest, "Unrouted connections");
        egui::CollapsingHeader::new("Visible layers")
            .default_open(true)
            .show(ui, |ui| {
                let names: Vec<_> = b
                    .root
                    .child("layers")
                    .into_iter()
                    .flat_map(|n| n.items().iter().skip(1))
                    .map(|n| n.value(1).to_owned())
                    .collect();
                for name in &names {
                    let mut on = self
                        .pcb
                        .visible_layers
                        .as_ref()
                        .is_none_or(|v| v.contains(name));
                    if ui.checkbox(&mut on, name).changed() {
                        let selected = self.pcb.visible_layers.get_or_insert_with(|| names.clone());
                        if on {
                            if !selected.contains(name) {
                                selected.push(name.clone());
                            }
                        } else {
                            selected.retain(|n| n != name);
                        }
                    }
                }
            });
        ui.separator();
        ui.label("Net for track, via or zone");
        egui::ComboBox::from_label("Net")
            .selected_text(&self.pcb.net)
            .show_ui(ui, |ui| {
                for (_, name) in b.nets() {
                    if !name.is_empty() {
                        ui.selectable_value(&mut self.pcb.net, name.clone(), name);
                    }
                }
            });
        ui.text_edit_singleline(&mut self.pcb.net);
        if let Some(i) = self.pcb.selected
            && let Some(n) = b.root.items().get(i)
        {
            ui.separator();
            ui.strong(format!("{} {}", n.name(), pcb::field(n, "Reference")));
            ui.label(n.value(1));
            let id = pcb::id(n);
            let at = pcb::point(n, "at");
            ui.label(format!(
                "X {:.2} · Y {:.2} mm · {:.0}°",
                at.0,
                at.1,
                pcb::angle(n)
            ));
            ui.add_enabled_ui(self.pcb.pending.is_none(),|ui|{ui.horizontal(|ui|{for(label,op)in[("Rotate 90°","rotate"),("Flip side","flip"),("Delete","delete")]{if ui.button(label).clicked(){self.pcb_job("pcb_edit",json!({"actions":[{"op":op,"uuid":id,"rotation":pcb::angle(n)+90.}]}),&ctx);}}});if n.name()=="footprint"{ui.label("Replacement footprint ID");ui.text_edit_singleline(&mut self.pcb.library_id);if ui.button("Preview safe replacement").clicked(){self.pcb_job("pcb_preview",json!({"actions":[{"op":"replace_footprint","uuid":id,"library_id":self.pcb.library_id}]}),&ctx);}for pad in b.pads().iter().filter(|p|p.object==i){ui.label(format!("Pad {} · {} · {}",pad.number,pad.net_name,pad.shape));}}});
        }
        ui.separator();
        egui::CollapsingHeader::new("Design rules & stackup").default_open(true).show(ui,|ui|{let mut r=b.rules.clone();ui.horizontal(|ui|{ui.label("Clearance mm");ui.add(egui::DragValue::new(&mut r.clearance).speed(0.05));});ui.horizontal(|ui|{ui.label("Min track mm");ui.add(egui::DragValue::new(&mut r.track_width).speed(0.05));});ui.horizontal(|ui|{ui.label("Via Ø / drill mm");ui.add(egui::DragValue::new(&mut r.via_diameter).speed(0.05));ui.add(egui::DragValue::new(&mut r.via_drill).speed(0.05));});if ui.button("Apply rules").clicked(){self.pcb_job("pcb_edit",json!({"rules":r}),&ctx);}for count in [2,4,6,8]{if ui.button(format!("{count} copper layers")).clicked(){self.pcb_job("pcb_edit",json!({"actions":[{"op":"stackup","copper_layers":count,"thickness_mm":b.thickness()}]}),&ctx);}}});
        ui.separator();
        ui.label("Place footprint");
        ui.text_edit_singleline(&mut self.pcb.library_id);
        ui.text_edit_singleline(&mut self.pcb.reference);
        ui.text_edit_singleline(&mut self.pcb.value);
        if ui.button("Place at view centre").clicked() {
            self.pcb_job("pcb_edit",json!({"actions":[{"op":"add_footprint","library_id":self.pcb.library_id,"reference":self.pcb.reference,"value":self.pcb.value,"at":self.pcb.view.center}]}),&ctx);
        }
        ui.label("Text to place");
        ui.text_edit_singleline(&mut self.pcb.text);
        ui.separator();
        egui::ComboBox::from_label("Export")
            .selected_text(&self.pcb.export_format)
            .show_ui(ui, |ui| {
                for f in [
                    "manufacturing",
                    "native",
                    "gerbers",
                    "drill",
                    "pos",
                    "bom",
                    "png",
                    "svg",
                    "pdf",
                    "step",
                    "glb",
                    "stl",
                    "vrml",
                    "brep",
                    "ply",
                ] {
                    ui.selectable_value(&mut self.pcb.export_format, f.into(), f);
                }
            });
        if ui.button("Export…").clicked() {
            let format = self.pcb.export_format.clone();
            let path =
                if ["manufacturing", "gerbers", "drill", "svg", "pdf"].contains(&format.as_str()) {
                    rfd::FileDialog::new().pick_folder()
                } else {
                    rfd::FileDialog::new()
                        .set_file_name(format!(
                            "Board.{}",
                            if format == "native" {
                                "kicad_pcb"
                            } else {
                                &format
                            }
                        ))
                        .save_file()
                };
            if let Some(path) = path {
                self.pcb_job("pcb_export", json!({"path":path,"format":format}), &ctx);
            }
        }
        if let Some(v) = self.pcb.result.clone() {
            ui.separator();
            ui.strong("Last operation");
            if let Some(id) = v["preview_id"].as_str() {
                ui.label(format!(
                    "{} added · {} changed",
                    v["changes"]["added"].as_array().map_or(0, Vec::len),
                    v["changes"]["changed"].as_array().map_or(0, Vec::len)
                ));
                if ui
                    .add_enabled(
                        v["can_commit"] == true && self.pcb.pending.is_none(),
                        egui::Button::new("Commit preview"),
                    )
                    .clicked()
                {
                    self.pcb_job("pcb_commit", json!({"preview_id":id}), &ctx);
                }
                if ui.button("Open preview image").clicked() {
                    if let Some(path) = v["preview_image_path"].as_str() {
                        let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
                    }
                }
            }
            egui::CollapsingHeader::new("Findings and exact results")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(
                            &mut serde_json::to_string_pretty(&v).unwrap_or_default(),
                        )
                        .desired_rows(12)
                        .interactive(false),
                    );
                });
        }
    }
    fn pcb_canvas(&mut self, ui: &mut egui::Ui) {
        let Some(b) = self.pcb.board.as_ref().map(Board::snapshot) else {
            return;
        };
        let revision = format!(
            "{}{:?}{}",
            b.revision(),
            self.pcb.visible_layers,
            self.pcb.show_ratsnest
        );
        if self
            .pcb
            .scene_cache
            .as_ref()
            .is_none_or(|(r, _)| *r != revision)
        {
            self.pcb.scene_cache = Some((
                revision,
                b.scene(self.pcb.visible_layers.as_deref(), self.pcb.show_ratsnest),
            ));
        }
        let scene = self.pcb.scene_cache.as_ref().unwrap().1.clone();
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        if self.pcb.fit {
            self.pcb.view.fit(&scene, rect);
            self.pcb.fit = false;
        }
        if response.hovered() {
            let zoom = ui.input(|i| i.zoom_delta());
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            self.pcb.view.scale =
                (self.pcb.view.scale * zoom * (scroll.y * 0.002).exp()).clamp(0.1, 200.);
            self.pcb.view.center.0 -= scroll.x as f64 / self.pcb.view.scale as f64;
        }
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            let delta = ui.input(|i| i.pointer.delta());
            self.pcb.view.center.0 -= delta.x as f64 / self.pcb.view.scale as f64;
            self.pcb.view.center.1 -= delta.y as f64 / self.pcb.view.scale as f64;
        }
        canvas::paint(
            &ui.painter_at(rect),
            &self.pcb.view,
            rect,
            &scene,
            self.pcb.selected,
            false,
        );
        let ctx = ui.ctx().clone();
        if !ctx.egui_wants_keyboard_input() && self.pcb.pending.is_none() {
            let command = ui.input(|i| i.modifiers.command);
            if ui.input(|i| i.key_pressed(Key::S)) && command {
                self.save_pcb();
            }
            if command && ui.input(|i| i.key_pressed(Key::Z)) {
                self.pcb_job(
                    if ui.input(|i| i.modifiers.shift) {
                        "pcb_redo"
                    } else {
                        "pcb_undo"
                    },
                    json!({}),
                    &ctx,
                );
            }
            if ui.input(|i| i.key_pressed(Key::Home)) {
                self.pcb.fit = true;
            }
            for (key, tool) in [
                (Key::M, "Move"),
                (Key::W, "Route"),
                (Key::V, "Via"),
                (Key::T, "Text"),
            ] {
                if ui.input(|i| i.key_pressed(key)) && !command {
                    self.pcb.tool = tool.into();
                }
            }
            if let Some(i) = self.pcb.selected {
                if let Some(n) = b.root.items().get(i) {
                    let op = if ui.input(|i| i.key_pressed(Key::Delete)) {
                        Some("delete")
                    } else if n.name() == "footprint" && ui.input(|i| i.key_pressed(Key::R)) {
                        Some("rotate")
                    } else if n.name() == "footprint" && ui.input(|i| i.key_pressed(Key::F)) {
                        Some("flip")
                    } else {
                        None
                    };
                    if let Some(op) = op {
                        self.pcb_job(
                            "pcb_edit",
                            json!({"actions":[{"op":op,"uuid":pcb::id(n)}]}),
                            &ctx,
                        );
                    }
                }
            }
        }
        if let Some(pos) = response.hover_pos() {
            let at = self.pcb.view.snap(self.pcb.view.world(pos, rect));
            self.cursor = at;
            if !self.pcb.points.is_empty() {
                let mut ps = self.pcb.points.clone();
                ps.push(at);
                ui.painter().add(egui::Shape::line(
                    ps.iter().map(|p| self.pcb.view.screen(*p, rect)).collect(),
                    Stroke::new(2_f32, ACCENT),
                ));
            }
            if response.clicked() && self.pcb.pending.is_none() {
                let pad = b
                    .pads()
                    .into_iter()
                    .min_by(|a, c| {
                        (a.at.0 - at.0)
                            .hypot(a.at.1 - at.1)
                            .total_cmp(&(c.at.0 - at.0).hypot(c.at.1 - at.1))
                    })
                    .filter(|p| (p.at.0 - at.0).hypot(p.at.1 - at.1) < 2.);
                match self.pcb.tool.as_str(){"Select"=>self.pcb.selected=canvas::hit(&scene,&self.pcb.view,at),"Move"=>{if let Some(i)=self.pcb.selected{self.pcb_job("pcb_edit",json!({"actions":[{"op":"move","uuid":pcb::id(&b.root.items()[i]),"at":at}]}),&ctx);self.pcb.tool="Select".into();}},"Route"=>{if let Some(p)=pad{let endpoint=json!({"reference":p.reference,"number":p.number});if let Some(start)=self.pcb.start.take(){self.pcb_job("pcb_connect",json!({"connections":[{"from":start,"to":endpoint,"width_mm":self.pcb.width}]}),&ctx);}else{self.pcb.start=Some(endpoint);self.pcb.net=p.net_name;}}},"Via"=>self.pcb_job("pcb_edit",json!({"actions":[{"op":"via","at":at,"net":self.pcb.net,"layers":b.copper_layers()}]}),&ctx),"Text"=>self.pcb_job("pcb_edit",json!({"actions":[{"op":"text","text":self.pcb.text,"at":at,"layer":"F.SilkS"}]}),&ctx),"Track"|"Outline"|"Zone"|"Keepout"=>{self.pcb.points.push(at);if response.double_clicked(){let points=std::mem::take(&mut self.pcb.points);let op=match self.pcb.tool.as_str(){"Track"=>"track","Outline"=>"outline","Zone"=>"zone",_=>"keepout"};self.pcb_job("pcb_edit",json!({"actions":[{"op":op,"points":points,"net":self.pcb.net,"layer":self.pcb.layer,"width_mm":self.pcb.width}]}),&ctx);}},_=>{}}
            }
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.pcb.points.clear();
            self.pcb.start = None;
            self.pcb.tool = "Select".into();
        }
        ui.painter().text(rect.left_bottom()+Vec2::new(12.,-16.),egui::Align2::LEFT_BOTTOM,"Middle drag: pan · wheel: zoom · Move: select then destination · double click: finish outline/track/zone",egui::FontId::monospace(11.),Color32::from_gray(70));
    }
    pub(super) fn pcb_dialogs(&mut self, ctx: &egui::Context) {
        self.footprint_dialog(ctx);
        if self.pcb.preview_dialog {
            let mut open = true;
            egui::Window::new("PCB engineering preview · review before commit").open(&mut open).default_size(Vec2::new(1050.,700.)).show(ctx,|ui|{if let Some(v)=self.pcb.result.clone(){ui.horizontal(|ui|{ui.strong(if v["can_commit"]==true{"Candidate is ready to commit"}else{"Candidate has failures · no edits applied"});if ui.add_enabled(v["can_commit"]==true&&self.pcb.pending.is_none(),egui::Button::new("Commit candidate")).clicked(){self.pcb_job("pcb_commit",json!({"preview_id":v["preview_id"]}),ctx);}});if let Some(texture)=&self.pcb.preview_texture{ui.add(egui::Image::new(texture).fit_to_exact_size(Vec2::new(ui.available_width(),420.)));}egui::ScrollArea::vertical().max_height(170.).show(ui,|ui|{ui.label(serde_json::to_string_pretty(&json!({"changes":v["changes"],"failures":v["failures"],"verification":v["verification"]})).unwrap_or_default());});}});
            self.pcb.preview_dialog = open;
        }

        self.archived_dialog(ctx);
        if !self.pcb.show_3d {
            return;
        }
        let mut open = true;
        egui::Window::new("3D assembly · native KiCad models")
            .open(&mut open)
            .default_size(Vec2::new(960., 680.))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Load / refresh full assembly").clicked() {
                        self.pcb_job("pcb_3d", json!({}), ctx);
                    }
                    if ui.button("Fit").clicked() {
                        self.pcb.zoom = 1.;
                        self.pcb.pan = Vec2::ZERO;
                    }
                    if ui.button("Top").clicked() {
                        self.pcb.pitch = std::f64::consts::FRAC_PI_2;
                        self.pcb.yaw = 0.;
                    }
                    ui.checkbox(&mut self.pcb.solid, "Shaded");
                    ui.label("Drag to orbit · right drag to pan · wheel to zoom");
                });
                if let Some(board)=&self.pcb.board{let models=jenga_tools::pcb_backend::model_findings(board);let absent=models.as_array().unwrap().iter().filter(|m|m["status"]!="available").count();if absent>0{ui.colored_label(ACCENT,format!("{absent} footprint model links are missing or unassigned · check Models in the board inspector"));}}
                let fallback = self.pcb.board.as_ref().map(Mesh::board_basic);
                if let Some(mesh) = self.pcb.mesh.as_ref().or(fallback.as_ref()) {
                    for w in &mesh.warnings {
                        ui.label(w);
                    }
                    let (rect, r) = ui.allocate_exact_size(
                        ui.available_size().max(Vec2::new(320., 300.)),
                        egui::Sense::drag(),
                    );
                    if r.dragged_by(egui::PointerButton::Primary) {
                        let d = ui.input(|i| i.pointer.delta());
                        self.pcb.yaw += d.x as f64 * 0.01;
                        self.pcb.pitch = (self.pcb.pitch + d.y as f64 * 0.01).clamp(-1.55, 1.55);
                    }
                    if r.dragged_by(egui::PointerButton::Secondary) {
                        self.pcb.pan += ui.input(|i| i.pointer.delta());
                    }
                    if r.hovered() {
                        self.pcb.zoom = (self.pcb.zoom
                            * ui.input(|i| i.zoom_delta())
                            * (ui.input(|i| i.smooth_scroll_delta.y) * 0.001).exp())
                        .clamp(0.1, 30.);
                    }
                    paint_mesh(
                        ui,
                        rect,
                        mesh,
                        self.pcb.yaw,
                        self.pcb.pitch,
                        self.pcb.zoom,
                        self.pcb.pan,
                        self.pcb.solid,
                    );
                } else {
                    ui.label("Open a PCB to view its assembly.");
                }
            });
        self.pcb.show_3d = open;
    }
}
pub(super) fn paint_mesh(
    ui: &egui::Ui,
    rect: egui::Rect,
    mesh: &Mesh,
    yaw: f64,
    pitch: f64,
    zoom: f32,
    pan: Vec2,
    solid: bool,
) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6., Color32::from_rgb(29, 34, 41));
    let (lo, hi) = mesh.bounds();
    let center = [
        (lo[0] + hi[0]) / 2.,
        (lo[1] + hi[1]) / 2.,
        (lo[2] + hi[2]) / 2.,
    ];
    let scale = rect.width().min(rect.height()) as f64
        / ((hi[0] - lo[0])
            .hypot(hi[1] - lo[1])
            .max(hi[2] - lo[2])
            .max(1.)
            * 1.25)
        * zoom as f64;
    let rotate = |v: [f64; 3]| {
        let x = v[0] - center[0];
        let y = v[1] - center[1];
        let z = v[2] - center[2];
        let a = x * yaw.cos() - y * yaw.sin();
        let b = x * yaw.sin() + y * yaw.cos();
        [
            a,
            b * pitch.sin() + z * pitch.cos(),
            b * pitch.cos() - z * pitch.sin(),
        ]
    };
    if solid {
        // Native assemblies contain large overlapping board, copper and mask
        // faces. Sorting whole triangles cannot resolve their per-pixel depth.
        // Cache a depth-buffered image, rebuilding only when mesh/view changes.
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for t in &mesh.triangles {
            for v in t.vertices {
                for n in v {
                    n.to_bits().hash(&mut hash);
                }
            }
            t.color.hash(&mut hash);
        }
        for n in [
            yaw,
            pitch,
            zoom as f64,
            pan.x as f64,
            pan.y as f64,
            rect.width() as f64,
            rect.height() as f64,
        ] {
            n.to_bits().hash(&mut hash);
        }
        let key = hash.finish();
        let id = ui.id().with("depth_buffered_assembly");
        let cached = ui
            .ctx()
            .data_mut(|d| d.get_temp::<(u64, egui::TextureHandle)>(id));
        let texture = if let Some((old, texture)) = cached.filter(|(old, _)| *old == key) {
            let _ = old;
            texture
        } else {
            let factor = (1200. / rect.width())
                .min(900. / rect.height())
                .min(1.)
                .max(0.01);
            let width = (rect.width() * factor).round().max(1.) as usize;
            let height = (rect.height() * factor).round().max(1.) as usize;
            let mut pixels = vec![Color32::from_rgb(29, 34, 41); width * height];
            let mut depths = vec![f64::INFINITY; width * height];
            let edge = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
                (p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])
            };
            for t in &mesh.triangles {
                let v = t.vertices.map(rotate);
                let p = v.map(|p| {
                    [
                        ((rect.width() / 2. + pan.x) as f64 + p[0] * scale) * factor as f64,
                        ((rect.height() / 2. + pan.y) as f64 - p[1] * scale) * factor as f64,
                    ]
                });
                let area = edge(p[0], p[1], p[2]);
                if area.abs() < 1e-9 {
                    continue;
                }
                let x0 = p
                    .iter()
                    .map(|p| p[0])
                    .fold(f64::INFINITY, f64::min)
                    .floor()
                    .max(0.) as usize;
                let y0 = p
                    .iter()
                    .map(|p| p[1])
                    .fold(f64::INFINITY, f64::min)
                    .floor()
                    .max(0.) as usize;
                let x1 = p
                    .iter()
                    .map(|p| p[0])
                    .fold(f64::NEG_INFINITY, f64::max)
                    .ceil()
                    .max(0.)
                    .min(width as f64) as usize;
                let y1 = p
                    .iter()
                    .map(|p| p[1])
                    .fold(f64::NEG_INFINITY, f64::max)
                    .ceil()
                    .max(0.)
                    .min(height as f64) as usize;
                let n = jenga_core::mesh::unit(jenga_core::mesh::cross(
                    jenga_core::mesh::sub(v[1], v[0]),
                    jenga_core::mesh::sub(v[2], v[0]),
                ));
                let light =
                    (0.45 + 0.55 * (n[0] * 0.3 + n[1] * 0.4 + n[2] * 0.85).abs()).clamp(0.3, 1.);
                let color = Color32::from_rgb(
                    (t.color[0] as f64 * light) as u8,
                    (t.color[1] as f64 * light) as u8,
                    (t.color[2] as f64 * light) as u8,
                );
                for y in y0..y1 {
                    for x in x0..x1 {
                        let at = [x as f64 + 0.5, y as f64 + 0.5];
                        let a = edge(p[1], p[2], at) / area;
                        let b = edge(p[2], p[0], at) / area;
                        let c = 1. - a - b;
                        if a < -1e-8 || b < -1e-8 || c < -1e-8 {
                            continue;
                        }
                        let depth = a * v[0][2] + b * v[1][2] + c * v[2][2];
                        let index = y * width + x;
                        if depth < depths[index] {
                            depths[index] = depth;
                            pixels[index] = color;
                        }
                    }
                }
            }
            let texture = ui.ctx().load_texture(
                "PCB assembly",
                egui::ColorImage::new([width, height], pixels),
                egui::TextureOptions::LINEAR,
            );
            ui.ctx()
                .data_mut(|d| d.insert_temp(id, (key, texture.clone())));
            texture
        };
        painter.image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            Color32::WHITE,
        );
        return;
    }
    let mut faces: Vec<_> = mesh
        .triangles
        .iter()
        .step_by((mesh.triangles.len() / 100000).max(1))
        .map(|t| {
            let v = t.vertices.map(rotate);
            let depth = v.iter().map(|p| p[2]).sum::<f64>();
            (depth, v, t.color)
        })
        .collect();
    faces.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut triangles = egui::Mesh::default();
    for (_, v, color) in faces {
        let ps: Vec<_> = v
            .iter()
            .map(|p| rect.center() + pan + Vec2::new((p[0] * scale) as f32, (-p[1] * scale) as f32))
            .collect();
        let n = jenga_core::mesh::unit(jenga_core::mesh::cross(
            jenga_core::mesh::sub(v[1], v[0]),
            jenga_core::mesh::sub(v[2], v[0]),
        ));
        let light = (0.45 + 0.55 * (n[0] * 0.3 + n[1] * 0.4 + n[2] * 0.85).abs()).clamp(0.3, 1.);
        let c = Color32::from_rgb(
            (color[0] as f64 * light) as u8,
            (color[1] as f64 * light) as u8,
            (color[2] as f64 * light) as u8,
        );
        if solid {
            let base = triangles.vertices.len() as u32;
            for p in ps {
                triangles.colored_vertex(p, c);
            }
            triangles.indices.extend([base, base + 1, base + 2]);
        } else {
            let mut edge = ps;
            edge.push(edge[0]);
            painter.add(egui::Shape::line(edge, Stroke::new(0.5_f32, c)));
        }
    }
    if solid {
        painter.add(egui::Shape::mesh(triangles));
    }
}
