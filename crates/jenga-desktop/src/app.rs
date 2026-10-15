mod chooser;
mod dialogs;
mod editor;
use crate::{
    canvas::{self, View},
    settings::Settings,
};
use eframe::egui::{self, Color32, Key, RichText, Stroke, Vec2};
use jenga_core::{
    checks::{self, Issue},
    document::{Document, create_project, editable, position, property},
    export,
    scene::{Point, Scene},
    sexpr::Node,
};
use jenga_libraries::{Libraries, Source};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

const ACCENT: Color32 = Color32::from_rgb(225, 35, 43);
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Select,
    Symbol,
    Wire,
    Junction,
    Label,
    Text,
    NoConnect,
    Move,
}
pub struct App {
    doc: Option<Document>,
    settings: Settings,
    view: View,
    selected: Option<usize>,
    tool: Tool,
    history: Vec<Node>,
    redo: Vec<Node>,
    status: String,
    error: Option<String>,
    libraries: Option<Libraries>,
    library_rx: Option<mpsc::Receiver<Libraries>>,
    search: String,
    chooser: bool,
    chooser_focus: bool,
    chosen_id: Option<String>,
    chosen_symbol: Option<Node>,
    unit: u32,
    library_filter: String,
    footprint_ids: Vec<String>,
    footprint_id: String,
    footprint_scene: Option<Scene>,
    footprint_search: String,
    repeat: bool,
    placement_rotation: u32,
    palette_search: String,
    project_search: String,
    recent_symbols: Vec<String>,
    expanded_libraries: std::collections::HashSet<String>,
    project_previews: std::collections::HashMap<PathBuf, (Option<std::time::SystemTime>, Scene)>,
    erc_errors_only: bool,
    wire_start: Option<Point>,
    text: String,
    text_dialog: bool,
    library_dialog: bool,
    about: bool,
    new_project: bool,
    project_name: String,
    project_folder: PathBuf,
    checks: Vec<Issue>,
    show_checks: bool,
    fit: bool,
    drag_previous: Option<Point>,
    canvas_rect: egui::Rect,
    cursor: Point,
    allow_close: bool,
    capture: Option<PathBuf>,
    capture_requested: bool,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.spacing.item_spacing = Vec2::new(8., 7.);
        style.visuals.panel_fill = Color32::from_rgb(248, 247, 242);
        style.visuals.window_fill = Color32::from_rgb(252, 251, 247);
        style.visuals.selection.bg_fill = Color32::from_rgb(250, 220, 218);
        style.visuals.override_text_color = Some(Color32::from_rgb(28, 32, 40));
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(249, 248, 244);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(249, 248, 244);
        style.visuals.widgets.inactive.bg_stroke =
            Stroke::new(1_f32, Color32::from_rgb(211, 210, 203));
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(250, 231, 228);
        style.spacing.button_padding = Vec2::new(12., 9.);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(15.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(15.));
        cc.egui_ctx.set_global_style(style);
        let settings = Settings::load();
        let grid = if settings.grid > 0. {
            settings.grid
        } else {
            1.27
        };
        let mut app = Self {
            doc: None,
            settings,
            view: View {
                grid,
                ..View::default()
            },
            selected: None,
            tool: Tool::Select,
            history: vec![],
            redo: vec![],
            status: "Ready".into(),
            error: None,
            libraries: None,
            library_rx: None,
            search: String::new(),
            chooser: false,
            chooser_focus: false,
            chosen_id: None,
            chosen_symbol: None,
            unit: 1,
            library_filter: String::new(),
            footprint_ids: vec![],
            footprint_id: String::new(),
            footprint_scene: None,
            footprint_search: String::new(),
            repeat: false,
            placement_rotation: 0,
            palette_search: String::new(),
            project_search: String::new(),
            recent_symbols: vec![],
            expanded_libraries: std::collections::HashSet::new(),
            project_previews: std::collections::HashMap::new(),
            erc_errors_only: false,
            wire_start: None,
            text: String::new(),
            text_dialog: false,
            library_dialog: false,
            about: false,
            new_project: false,
            project_name: "MyCircuit".into(),
            project_folder: std::env::var_os("USERPROFILE")
                .map(|p| PathBuf::from(p).join("Documents"))
                .unwrap_or_default(),
            checks: vec![],
            show_checks: false,
            fit: true,
            drag_previous: None,
            canvas_rect: egui::Rect::NOTHING,
            cursor: (0., 0.),
            allow_close: false,
            capture: None,
            capture_requested: false,
        };
        app.capture =
            std::env::args().find_map(|s| s.strip_prefix("--capture=").map(PathBuf::from));
        if let Some(path) = open {
            app.open_path(&path);
        }
        if app.library_rx.is_none() {
            app.reload_libraries();
        }
        app
    }
    fn reload_libraries(&mut self) {
        let extra = self.settings.libraries.clone();
        let project = self
            .doc
            .as_ref()
            .and_then(|d| d.path.as_ref())
            .and_then(|p| p.parent())
            .map(Path::to_owned);
        let (tx, rx) = mpsc::channel();
        self.library_rx = Some(rx);
        self.status = "Indexing symbol libraries…".into();
        std::thread::spawn(move || {
            let mut libs = Libraries::discover(&extra, project.as_deref());
            if let Err(e) = libs.index() {
                libs.warnings.push(e.to_string());
            }
            let _ = tx.send(libs);
        });
    }
    fn checkpoint(&mut self) {
        if let Some(doc) = &self.doc {
            self.history.push(doc.root.clone());
            if self.history.len() > 100 {
                self.history.remove(0);
            }
            self.redo.clear();
            self.checks.clear();
        }
    }
    fn undo(&mut self) {
        if let (Some(root), Some(doc)) = (self.history.pop(), self.doc.as_mut()) {
            self.redo.push(doc.root.clone());
            doc.root = root;
            doc.dirty = true;
            self.selected = None;
            self.checks.clear();
        }
    }
    fn redo(&mut self) {
        if let (Some(root), Some(doc)) = (self.redo.pop(), self.doc.as_mut()) {
            self.history.push(doc.root.clone());
            doc.root = root;
            doc.dirty = true;
            self.selected = None;
            self.checks.clear();
        }
    }
    fn fail(&mut self, e: impl ToString) {
        self.error = Some(e.to_string());
    }
    fn save(&mut self, save_as: bool) -> bool {
        let Some(doc) = &self.doc else {
            return true;
        };
        let path = if !save_as { doc.path.clone() } else { None }.or_else(|| {
            rfd::FileDialog::new()
                .add_filter("KiCad schematic", &["kicad_sch"])
                .set_file_name(format!("{}.kicad_sch", doc.title()))
                .save_file()
        });
        let Some(mut path) = path else {
            return false;
        };
        if path.extension().is_none() {
            path.set_extension("kicad_sch");
        }
        match self.doc.as_mut().unwrap().save(&path) {
            Ok(()) => {
                self.settings.remember(path);
                self.persist();
                self.status = "Saved · previous file backed up".into();
                true
            }
            Err(e) => {
                self.fail(e);
                false
            }
        }
    }
    fn persist(&mut self) {
        if self.capture.is_some() {
            return;
        }
        self.settings.schema_version = 1;
        self.settings.grid = self.view.grid;
        if let Err(e) = self.settings.save() {
            self.fail(format!("Could not save settings: {e}"));
        }
    }
    fn may_replace(&mut self) -> bool {
        if !self.doc.as_ref().is_some_and(|d| d.dirty) {
            return true;
        }
        match rfd::MessageDialog::new()
            .set_title("Unsaved schematic")
            .set_description("Save your changes before continuing?")
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show()
        {
            rfd::MessageDialogResult::Yes => self.save(false),
            rfd::MessageDialogResult::No => true,
            _ => false,
        }
    }
    fn set_doc(&mut self, doc: Document) {
        if let Some(path) = &doc.path {
            self.settings.remember(path.clone());
        }
        self.doc = Some(doc);
        self.selected = None;
        self.history.clear();
        self.redo.clear();
        self.checks.clear();
        self.tool = Tool::Select;
        self.wire_start = None;
        self.fit = true;
        self.persist();
        self.reload_libraries();
    }
    fn open_path(&mut self, path: &Path) {
        let path = if path.extension().is_some_and(|e| e == "kicad_pro") {
            path.with_extension("kicad_sch")
        } else {
            path.to_owned()
        };
        match Document::open(&path) {
            Ok(doc) => self.set_doc(doc),
            Err(e) => self.fail(e),
        }
    }
    fn open(&mut self) {
        if !self.may_replace() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "KiCad projects and schematics",
                &["kicad_pro", "kicad_sch", "sch"],
            )
            .pick_file()
        {
            self.open_path(&path);
        }
    }
    fn choose_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.wire_start = None;
        match tool {
            Tool::Symbol => {
                self.chooser = true;
                self.chooser_focus = true;
            }
            Tool::Text | Tool::Label => {
                self.text.clear();
                self.text_dialog = true;
            }
            _ => {}
        }
    }
    fn inspect(&mut self) {
        if let Some(doc) = &self.doc {
            self.checks = checks::run(doc);
            self.show_checks = true;
            self.status = format!("Basic single-sheet ERC: {} issue(s)", self.checks.len());
        }
    }
    fn output(&mut self, kind: &str) {
        let Some(doc) = &self.doc else {
            return;
        };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(kind, &[kind])
            .set_file_name(format!("{}.{}", doc.title(), kind))
            .save_file()
        {
            let result = match kind {
                "svg" => export::svg(doc, &path),
                "pdf" => export::pdf(doc, &path),
                _ => export::bom(doc, &path),
            };
            match result {
                Ok(()) => self.status = format!("Exported {}", path.display()),
                Err(e) => self.fail(e),
            }
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let command = ctx.input(|i| i.modifiers.command);
        let shift = ctx.input(|i| i.modifiers.shift);
        let pressed = |key| ctx.input(|i| i.key_pressed(key));
        if pressed(Key::Escape) {
            self.chooser = false;
            self.text_dialog = false;
            self.library_dialog = false;
            self.new_project = false;
            self.tool = Tool::Select;
            self.wire_start = None;
            self.selected = None;
            return;
        }
        if self.chooser
            || self.text_dialog
            || self.library_dialog
            || self.new_project
            || self.about
            || self.error.is_some()
        {
            return;
        }
        if command && pressed(Key::O) {
            self.open();
        }
        if command && pressed(Key::S) {
            self.save(shift);
        }
        if command && pressed(Key::N) && self.may_replace() {
            self.new_project = true;
        }
        if ctx.egui_wants_keyboard_input()
            || self.chooser
            || self.text_dialog
            || self.library_dialog
            || self.new_project
        {
            return;
        }
        if command && pressed(Key::Z) {
            if shift {
                self.redo();
            } else {
                self.undo();
            }
        }
        if command && pressed(Key::Y) {
            self.redo();
        }
        if command || self.doc.is_none() {
            return;
        }
        if pressed(Key::Escape) {
            self.tool = Tool::Select;
            self.wire_start = None;
            self.selected = None;
        }
        if pressed(Key::A) {
            self.choose_tool(Tool::Symbol);
        }
        if pressed(Key::W) {
            self.choose_tool(Tool::Wire);
        }
        if pressed(Key::L) {
            self.choose_tool(Tool::Label);
        }
        if pressed(Key::T) {
            self.choose_tool(Tool::Text);
        }
        if pressed(Key::J) {
            self.choose_tool(Tool::Junction);
        }
        if pressed(Key::Q) {
            self.choose_tool(Tool::NoConnect);
        }
        if pressed(Key::M) {
            self.choose_tool(Tool::Move);
        }
        if pressed(Key::Home) {
            self.fit = true;
        }
        if pressed(Key::R) && self.tool == Tool::Symbol {
            self.placement_rotation = (self.placement_rotation + 1) % 4;
        } else if pressed(Key::R)
            && let Some(index) = self.selected
        {
            self.checkpoint();
            self.doc.as_mut().unwrap().rotate(index);
        }
        if (pressed(Key::Delete) || pressed(Key::Backspace))
            && let Some(index) = self.selected.take()
        {
            self.checkpoint();
            self.doc.as_mut().unwrap().remove(index);
        }
    }
}
fn paint_overlay(painter: &egui::Painter, view: &View, rect: egui::Rect, scene: &Scene) {
    // Ghost geometry must never clear wires or symbols underneath it.
    let (a, b) = scene.content_bounds();
    let area = egui::Rect::from_two_pos(view.screen(a, rect), view.screen(b, rect))
        .expand(2.)
        .intersect(rect);
    canvas::paint_shapes(&painter.with_clip_rect(area), view, rect, scene);
}
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(rx) = &self.library_rx {
            if let Ok(libs) = rx.try_recv() {
                self.status = format!("{} symbols ready", libs.entries.len());
                self.footprint_ids = libs.footprint_ids();
                self.libraries = Some(libs);
                self.library_rx = None;
                if self.capture.is_some() {
                    if std::env::args().any(|s| s == "--capture-chooser") {
                        if self.doc.is_none() {
                            self.doc = Some(Document::new());
                        }
                        self.search = "resistor".into();
                        self.chooser = true;
                        self.pick_symbol("Device:R");
                        self.footprint_id = "Resistor_SMD:R_0805_2012Metric".into();
                        self.load_footprint_preview();
                    }
                    if std::env::args().any(|s| s == "--capture-erc") {
                        self.inspect();
                    }
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        self.shortcuts(&ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.may_replace() {
                self.persist();
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        let title = self
            .doc
            .as_ref()
            .map(|d| format!("{}{} — Jenga", d.title(), if d.dirty { " *" } else { "" }))
            .unwrap_or_else(|| "Jenga — Projects".into());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        egui::Panel::top("menu").show_inside(ui, |ui| self.menu(ui));
        egui::Panel::bottom("status").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "X {:.2}  Y {:.2} mm    Grid {} mm    Zoom {:.0}%",
                        self.cursor.0,
                        self.cursor.1,
                        self.view.grid,
                        self.view.scale * 25.
                    ));
                });
            });
        });
        if self.doc.is_some() {
            egui::Panel::top("toolbar").show_inside(ui, |ui| self.toolbar(ui));
            if self.doc.is_some() {
                egui::Panel::right("properties")
                    .default_size(260.)
                    .min_size(220.)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.properties(ui));
                    });
                egui::Panel::left("components")
                    .default_size(235.)
                    .min_size(200.)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.palette(ui));
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(ui, |ui| {
                        if self.doc.is_some() {
                            self.canvas(ui);
                        }
                    });
            }
        } else {
            egui::CentralPanel::default().show_inside(ui, |ui| self.home(ui));
        }
        self.dialogs(&ctx);
        if let Some(path) = self.capture.clone() {
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    let result = image::save_buffer(
                        &path,
                        &bytes,
                        image.size[0] as u32,
                        image.size[1] as u32,
                        image::ColorType::Rgba8,
                    );
                    if let Err(e) = result {
                        self.fail(format!("Capture failed: {e}"));
                    } else {
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
            if self.libraries.is_some() && !self.capture_requested {
                self.capture_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}
