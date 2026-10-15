mod chooser;
mod control;
mod dialogs;
mod editor;
mod home;
mod support;
mod symbols;
mod transfer;
mod workspace;
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
    workspace_ui: workspace::WorkspaceUi,
    symbol_editor: Option<symbols::SymbolEditor>,
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
    check_details: Vec<checks::Finding>,
    show_checks: bool,
    active_issue: Option<checks::Finding>,
    page_dialog: bool,
    history_dialog: bool,
    rename_dialog: bool,
    rename_name: String,
    rename_project: bool,
    rename_folder: bool,
    fit: bool,
    drag_previous: Option<Point>,
    canvas_rect: egui::Rect,
    cursor: Point,
    allow_close: bool,
    capture: Option<PathBuf>,
    capture_requested: bool,
    control: Option<jenga_tools::transport::Server>,
    live_capture: Option<(PathBuf, mpsc::Sender<serde_json::Value>)>,
    observed_doc: Option<Document>,
    pending_human: Option<(Document, Document, u128)>,
    control_drag: Option<Point>,
    toast_until: u128,
    logo: egui::TextureHandle,
    home_projects: bool,
    connect_dialog: bool,
    help_dialog: bool,
    assistant_client: String,
    assistant_mode: String,
    assistant_workspace: String,
    setup_status: String,
    sync_at: u128,
    seen_project_events: std::collections::HashSet<String>,
    external_conflict: bool,
    reduced_motion: bool,
    copy_all: bool,
    copy_selection: Vec<String>,
    selection_add: bool,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.spacing.item_spacing = Vec2::new(6., 5.);
        style.visuals.panel_fill = Color32::from_rgb(248, 247, 242);
        style.visuals.window_fill = Color32::from_rgb(252, 251, 247);
        style.visuals.selection.bg_fill = Color32::from_rgb(250, 220, 218);
        style.visuals.override_text_color = Some(Color32::from_rgb(28, 32, 40));
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(249, 248, 244);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(249, 248, 244);
        style.visuals.widgets.inactive.bg_stroke =
            Stroke::new(1_f32, Color32::from_rgb(211, 210, 203));
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(250, 231, 228);
        style.spacing.button_padding = Vec2::new(8., 4.);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::monospace(13.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::monospace(13.));
        cc.egui_ctx.set_global_style(style);
        let logo = image::load_from_memory(include_bytes!("../../../resources/branding/jenga.png"))
            .expect("Jenga logo")
            .into_rgba8();
        let logo = cc.egui_ctx.load_texture(
            "jenga_loading",
            egui::ColorImage::from_rgba_unmultiplied(
                [logo.width() as usize, logo.height() as usize],
                logo.as_raw(),
            ),
            egui::TextureOptions::LINEAR,
        );
        let settings = Settings::load();
        let grid = if settings.grid > 0. {
            settings.grid
        } else {
            1.27
        };
        let mut app = Self {
            workspace_ui: Default::default(),
            symbol_editor: None,
            doc: None,
            settings,
            view: View {
                grid,
                ..View::default()
            },
            selected: None,
            copy_all: false,
            copy_selection: vec![],
            selection_add: false,
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
            check_details: vec![],
            show_checks: false,
            active_issue: None,
            page_dialog: false,
            history_dialog: false,
            rename_dialog: false,
            rename_name: String::new(),
            rename_project: false,
            rename_folder: false,
            fit: true,
            drag_previous: None,
            canvas_rect: egui::Rect::NOTHING,
            cursor: (0., 0.),
            allow_close: false,
            capture: None,
            capture_requested: false,
            control: None,
            live_capture: None,
            observed_doc: None,
            pending_human: None,
            control_drag: None,
            toast_until: 0,
            logo,
            home_projects: false,
            connect_dialog: false,
            help_dialog: false,
            assistant_client: "Codex".into(),
            assistant_mode: "auto".into(),
            assistant_workspace: std::env::var("USERPROFILE")
                .map(|p| format!("{p}\\Documents"))
                .unwrap_or_default(),
            setup_status: String::new(),
            sync_at: 0,
            seen_project_events: std::collections::HashSet::new(),
            external_conflict: false,
            reduced_motion: false,
        };
        app.capture =
            std::env::args().find_map(|s| s.strip_prefix("--capture=").map(PathBuf::from));
        app.connect_dialog = std::env::args().any(|s| s == "--connect-assistant");
        app.help_dialog = std::env::args().any(|s| s == "--help-features");
        app.reduced_motion = app.settings.reduced_motion;
        if let Some(path) = open {
            app.open_path(&path);
        }
        if std::env::args().any(|s| s == "--capture-project") {
            app.workspace_ui.show = true;
        }
        if std::env::args().any(|s| s == "--capture-symbol-editor") {
            app.symbol_editor = Some(Default::default());
        }
        if app.library_rx.is_none() {
            app.reload_libraries();
        }
        if app.capture.is_none() {
            let ctx = cc.egui_ctx.clone();
            match jenga_tools::transport::Server::start(std::sync::Arc::new(move || {
                ctx.request_repaint()
            })) {
                Ok(server) => app.control = Some(server),
                Err(e) => app.status = format!("External control unavailable: {e}"),
            }
        }
        app.observed_doc = app.doc.clone();
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
            self.check_details.clear();
            self.active_issue = None;
        }
    }
    fn undo(&mut self) {
        if let (Some(root), Some(doc)) = (self.history.pop(), self.doc.as_mut()) {
            self.redo.push(doc.root.clone());
            doc.root = root;
            doc.dirty = true;
            self.selected = None;
            self.checks.clear();
            self.check_details.clear();
            self.active_issue = None;
        }
    }
    fn redo(&mut self) {
        if let (Some(root), Some(doc)) = (self.redo.pop(), self.doc.as_mut()) {
            self.history.push(doc.root.clone());
            doc.root = root;
            doc.dirty = true;
            self.selected = None;
            self.checks.clear();
            self.check_details.clear();
            self.active_issue = None;
        }
    }
    fn fail(&mut self, e: impl ToString) {
        self.error = Some(e.to_string());
    }
    fn save(&mut self, save_as: bool) -> bool {
        self.observe_history();
        self.flush_human_history();
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
        let before = self.doc.as_ref().unwrap().clone();
        match self.doc.as_mut().unwrap().save(&path) {
            Ok(()) => {
                if let Err(e) = jenga_tools::journal::record(
                    &before,
                    self.doc.as_ref().unwrap(),
                    "user",
                    "save",
                ) {
                    self.fail(format!("Saved, but history failed: {e}"));
                }
                self.settings.remember(path);
                let _ = jenga_tools::catalog::record(
                    "save",
                    None,
                    self.doc.as_ref().and_then(|d| d.path.as_deref()),
                );
                self.external_conflict = false;
                self.persist();
                self.status = "Saved · previous file backed up".into();
                self.toast_until = jenga_tools::journal::now() + 3000;
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
        self.settings.reduced_motion = self.reduced_motion;
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
    fn set_doc(&mut self, mut doc: Document) {
        if let Some(path) = &doc.path {
            if let Ok(project) = jenga_tools::project_workspace::discover(path) {
                doc.page_context = project
                    .pages
                    .iter()
                    .position(|p| jenga_tools::catalog::same_path(&p.file, path))
                    .map(|i| (i + 1, project.pages.len()));
                self.workspace_ui.root = project
                    .project_file
                    .or_else(|| project.pages.first().map(|p| p.file.clone()));
            }
        }
        self.copy_all = false;
        self.copy_selection.clear();
        self.observe_history();
        self.flush_human_history();
        if let Some(path) = &doc.path {
            self.settings.remember(path.clone());
            let _ = jenga_tools::catalog::record("open", None, Some(path));
        }
        self.external_conflict = false;
        self.doc = Some(doc);
        self.observed_doc = self.doc.clone();
        self.selected = None;
        self.history.clear();
        self.redo.clear();
        self.checks.clear();
        self.check_details.clear();
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
            let scene = Scene::from_document(doc);
            self.check_details = self
                .checks
                .iter()
                .map(|i| checks::describe_with_scene(doc, i, &scene))
                .collect();
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
                "xlsx" => export::bom_xlsx(doc, &path),
                "kicad_sch" => export::schematic(doc, &path),
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
            self.copy_selection.clear();
            self.copy_all = false;
            self.chooser = false;
            self.text_dialog = false;
            self.library_dialog = false;
            self.new_project = false;
            self.connect_dialog = false;
            self.help_dialog = false;
            self.page_dialog = false;
            self.history_dialog = false;
            self.rename_dialog = false;
            self.show_checks = false;
            self.tool = Tool::Select;
            self.wire_start = None;
            self.selected = None;
            return;
        }
        if self.chooser
            || self.text_dialog
            || self.library_dialog
            || self.new_project
            || self.connect_dialog
            || self.help_dialog
            || self.page_dialog
            || self.history_dialog
            || self.rename_dialog
            || self.show_checks
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
        if command && pressed(Key::A) {
            self.copy_all = true;
            self.status = "All schematic objects selected · Ctrl+C to copy".into();
        }
        if (command && pressed(Key::C))
            || ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)))
        {
            self.copy_clipboard(ctx);
        }
        if (command && pressed(Key::X))
            || ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Cut)))
        {
            self.cut_clipboard(ctx);
        }
        if (command && pressed(Key::V))
            || ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))))
        {
            self.paste_clipboard();
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
        self.poll_control(&ctx);
        self.poll_project_changes(&ctx);
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
        if self.doc.is_some() && self.workspace_ui.show {
            egui::CentralPanel::default().show_inside(ui, |ui| self.project_overview(ui));
        } else if self.doc.is_some() {
            egui::Panel::top("toolbar").show_inside(ui, |ui| self.toolbar(ui));
            if self.doc.is_some() {
                egui::Panel::right("properties_compact_v3")
                    .default_size(218.)
                    .min_size(180.)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.properties(ui));
                    });
                egui::Panel::left("components_compact_v3")
                    .default_size(205.)
                    .min_size(175.)
                    .show_inside(ui, |ui| {
                        egui::Panel::bottom("fixed_placement_tools")
                            .show_inside(ui, |ui| self.palette_footer(ui));
                        self.palette(ui);
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
            egui::Panel::left("home_navigation")
                .default_size(175.)
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.add_space(18.);
                    crate::design::brand(ui, "JENGA", 1.1);
                    ui.add_space(40.);
                    for label in [
                        "⌂  Home",
                        "□  Projects",
                        "▤  Libraries",
                        "Connect Assistant",
                        "Help",
                        "⚙  Settings",
                    ] {
                        if ui
                            .add_sized(
                                [ui.available_width(), 38.],
                                egui::Button::new(label).selected(label.contains("Home")),
                            )
                            .clicked()
                        {
                            if label == "Connect Assistant" {
                                self.connect_dialog = true;
                            } else if label == "Help" {
                                self.help_dialog = true;
                            } else if label.contains("Libraries") || label.contains("Settings") {
                                self.library_dialog = true;
                            } else if label.contains("Projects") {
                                self.home_projects = true;
                            } else {
                                self.project_search.clear();
                            }
                        }
                    }
                    ui.add_space(40.);
                    ui.label(
                        RichText::new(
                            "Engineering
the next thing.",
                        )
                        .small()
                        .monospace()
                        .weak(),
                    );
                    ui.label(
                        RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .small()
                            .weak(),
                    );
                });
            egui::CentralPanel::default().show_inside(ui, |ui| self.home(ui));
        }
        self.dialogs(&ctx);
        self.observe_history();
        if self.pending_human.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(550));
        }
        if self.toast_until > jenga_tools::journal::now() {
            egui::Area::new(egui::Id::new("notification"))
                .anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-18., -38.))
                .interactable(false)
                .show(&ctx, |ui| {
                    egui::Frame::group(ui.style())
                        .fill(Color32::from_rgb(253, 252, 248))
                        .inner_margin(12.)
                        .show(ui, |ui| {
                            ui.label(RichText::new(&self.status).color(ACCENT));
                        });
                });
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
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
