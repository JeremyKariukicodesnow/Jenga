//! External tool control. There is deliberately no AI service or chat interface.
use super::*;
use anyhow::{Context, Result, ensure};
use jenga_tools::{engine, journal};
use serde_json::{Value, json};
impl App {
    pub(super) fn poll_control(&mut self, ctx: &egui::Context) {
        let mut jobs = vec![];
        if let Some(server) = &self.control {
            while let Ok(job) = server.jobs.try_recv() {
                jobs.push(job);
            }
        }
        for job in jobs {
            self.flush_human_history();
            let method = job.request["method"].as_str().unwrap_or("");
            let p = job.request.get("params").cloned().unwrap_or(json!({}));
            if method == "screenshot" && p.get("interface") == Some(&Value::Bool(true)) {
                if self.live_capture.is_some() {
                    let _ = job
                        .reply
                        .send(json!({"error":"A screenshot is already pending"}));
                    continue;
                }
                match engine::string(&p, "path") {
                    Ok(path) => {
                        self.live_capture = Some((PathBuf::from(path), job.reply));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                            egui::UserData::default(),
                        ));
                    }
                    Err(e) => {
                        let _ = job.reply.send(json!({"error":e.to_string()}));
                    }
                }
                continue;
            }
            let result = self.control_request(method, &p);
            let _ = job.reply.send(match result {
                Ok(result) => json!({"result":result}),
                Err(e) => json!({"error":e.to_string()}),
            });
        }
        if let Some((path, reply)) = self.live_capture.take() {
            let screenshot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(image) = screenshot {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                let result = image::save_buffer(
                    &path,
                    &bytes,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                );
                let _ = reply.send(match result {
                    Ok(()) => json!({"result":{"path":path,"interface":true}}),
                    Err(e) => json!({"error":e.to_string()}),
                });
            } else {
                self.live_capture = Some((path, reply));
                ctx.request_repaint_after(std::time::Duration::from_millis(40));
            }
        }
    }
    pub(super) fn control_request(&mut self, method: &str, p: &Value) -> Result<Value> {
        if method == "ui_state" {
            return Ok(
                json!({"running_app":true,"pid":std::process::id(),"version":env!("CARGO_PKG_VERSION"),"document":self.doc.as_ref().map(engine::inspect),"selection":self.selected.and_then(|i|self.doc.as_ref()?.root.items().get(i)?.child("uuid")).map(|n|n.value(1)),"tool":self.tool.name(),"cursor_mm":self.cursor,"view":{"center":self.view.center,"scale":self.view.scale,"grid":self.view.grid,"show_grid":self.view.show_grid,"canvas_pixels":[self.canvas_rect.min.x,self.canvas_rect.min.y,self.canvas_rect.width(),self.canvas_rect.height()]},"dialogs":{"chooser":self.chooser,"page":self.page_dialog,"history":self.history_dialog,"rename":self.rename_dialog,"text":self.text_dialog,"libraries":self.library_dialog,"erc":self.show_checks},"indexing":self.library_rx.is_some(),"status":self.status}),
            );
        }
        if method == "ui_action" {
            match engine::string(p, "action")? {
                "dismiss_dialogs" => {
                    self.chooser = false;
                    self.text_dialog = false;
                    self.library_dialog = false;
                    self.new_project = false;
                    self.page_dialog = false;
                    self.history_dialog = false;
                    self.rename_dialog = false;
                    self.show_checks = false;
                    self.about = false;
                    self.error = None;
                    self.tool = Tool::Select;
                    self.wire_start = None;
                }
                "select" => {
                    let doc = self.doc.as_ref().context("No document open")?;
                    let i = engine::target(doc, p)?;
                    self.selected = Some(i);
                    if let Some((a, b)) = Scene::from_document(doc).object_bounds(i) {
                        self.view.center = ((a.0 + b.0) / 2., (a.1 + b.1) / 2.);
                    }
                }
                "show_chooser" => self.choose_tool(Tool::Symbol),
                "show_page_settings" => self.page_dialog = true,
                "show_history" => self.history_dialog = true,
                "show_erc" => self.inspect(),
                "focus_issue" => {
                    self.inspect();
                    let i = p
                        .get("index")
                        .and_then(Value::as_u64)
                        .context("Missing zero-based issue index")?
                        as usize;
                    let issue = self
                        .check_details
                        .get(i)
                        .context("Issue index out of range")?
                        .clone();
                    self.selected = issue.object;
                    if let Some(at) = issue.location {
                        self.view.center = at;
                        self.view.scale = 12.;
                    }
                    self.active_issue = Some(issue);
                    self.show_checks = false;
                }
                _ => anyhow::bail!("Unknown UI action"),
            }
            return Ok(json!({"ok":true}));
        }
        if method == "set_view" {
            if p.get("center").is_some() {
                self.view.center = engine::point(p, "center")?;
            }
            if let Some(scale) = p.get("scale").and_then(Value::as_f64) {
                ensure!(
                    scale.is_finite() && (0.5..=60.).contains(&scale),
                    "Invalid scale"
                );
                self.view.scale = scale as f32;
            }
            if let Some(grid) = p.get("grid").and_then(Value::as_f64) {
                ensure!((0.01..=10.).contains(&grid), "Invalid grid");
                self.view.grid = grid;
            }
            if let Some(show) = p.get("show_grid").and_then(Value::as_bool) {
                self.view.show_grid = show;
            }
            if p.get("fit") == Some(&Value::Bool(true)) {
                self.fit = true;
            }
            return Ok(json!({"ok":true}));
        }
        if method == "pointer" {
            return self.control_pointer(p);
        }
        if method == "undo" || method == "redo" {
            let before = self.doc.clone().context("No document open")?;
            if method == "undo" {
                ensure!(!self.history.is_empty(), "No undo history");
                self.undo();
            } else {
                ensure!(!self.redo.is_empty(), "No redo history");
                self.redo();
            }
            journal::record(
                &before,
                self.doc.as_ref().unwrap(),
                p.get("actor").and_then(Value::as_str).unwrap_or("external"),
                method,
            )?;
            self.observed_doc = self.doc.clone();
            return Ok(engine::inspect(self.doc.as_ref().unwrap()));
        }
        ensure!(
            method != "edit" || self.library_rx.is_none(),
            "Library indexing is still running; retry when ui_state.indexing is false"
        );
        let workspace = self
            .doc
            .as_ref()
            .and_then(|d| d.path.as_ref())
            .and_then(|p| p.parent())
            .map(Path::to_owned)
            .unwrap_or(std::env::current_dir()?);
        let mut engine = engine::Engine::new(workspace);
        engine.doc = self.doc.clone();
        engine.libraries = self.libraries.take();
        let before = self.doc.clone();
        let result = engine.request(method, p);
        self.libraries = engine.libraries;
        if result.is_ok() {
            if let Some(before) = before
                && method == "edit"
                && engine.doc.as_ref().is_some_and(|d| d.root != before.root)
            {
                self.history.push(before.root);
                self.redo.clear();
                self.checks.clear();
                self.check_details.clear();
                self.active_issue = None;
                self.selected = None;
            }
            self.doc = engine.doc;
            self.observed_doc = self.doc.clone();
            if matches!(
                method,
                "new" | "open" | "create_project" | "create_schematic" | "rename" | "close"
            ) {
                self.selected = None;
                self.history.clear();
                self.redo.clear();
                self.fit = true;
                self.checks.clear();
                self.check_details.clear();
                self.active_issue = None;
                self.reload_libraries();
            }
            if matches!(
                method,
                "open" | "create_project" | "create_schematic" | "rename" | "save"
            ) && let Some(path) = self.doc.as_ref().and_then(|d| d.path.clone())
            {
                self.settings.remember(path);
                self.persist();
            }
            if matches!(
                method,
                "import_library" | "write_library" | "create_library"
            ) {
                self.reload_libraries();
            }
            if matches!(
                method,
                "edit"
                    | "save"
                    | "new"
                    | "open"
                    | "create_project"
                    | "import_library"
                    | "write_library"
                    | "create_library"
                    | "rename"
                    | "create_schematic"
            ) {
                self.status = format!("External tools · {method} completed");
                self.toast_until = journal::now() + 3500;
            }
        }
        result
    }
    fn control_pointer(&mut self, p: &Value) -> Result<Value> {
        let before = self
            .doc
            .clone()
            .context("Open a schematic before using pointer events")?;
        if let Some(expected) = p.get("expected_revision").and_then(Value::as_str) {
            ensure!(expected == journal::revision(&before), "Revision conflict");
        }
        ensure!(
            !self.chooser
                && !self.text_dialog
                && !self.library_dialog
                && !self.new_project
                && !self.page_dialog
                && !self.history_dialog
                && !self.rename_dialog
                && !self.show_checks
                && !self.about
                && self.error.is_none(),
            "Dismiss the open dialog before pointer control"
        );
        let mut at = engine::point(p, "at")?;
        if p.get("space").and_then(Value::as_str) == Some("pixels") {
            at = self
                .view
                .world(egui::Pos2::new(at.0 as f32, at.1 as f32), self.canvas_rect);
        }
        if let Some(tool) = p.get("tool").and_then(Value::as_str) {
            self.tool = Tool::parse(tool).context("Unknown tool")?;
            self.wire_start = None;
        }
        if self.tool == Tool::Symbol
            && let Some(id) = p.get("library_id").and_then(Value::as_str)
        {
            self.chosen_symbol = Some(
                self.libraries
                    .as_mut()
                    .context("Libraries are loading")?
                    .symbol(id)?,
            );
            self.chosen_id = Some(id.into());
            self.footprint_id = self
                .chosen_symbol
                .as_ref()
                .map(|d| property(d, "Footprint").to_owned())
                .unwrap_or_default();
            self.placement_rotation = 0;
            self.unit = p.get("unit").and_then(Value::as_u64).unwrap_or(1) as u32;
        }
        if let Some(text) = p.get("text").and_then(Value::as_str) {
            self.text = text.into();
        }
        self.cursor = self.view.snap(at);
        let secondary = p.get("button").and_then(Value::as_str) == Some("secondary");
        let phase = p.get("phase").and_then(Value::as_str).unwrap_or("click");
        match phase {
            "move" => {
                if let (Some(i), Some(previous)) = (self.selected, self.control_drag) {
                    let next = self.view.snap(at);
                    self.doc.as_mut().unwrap().translate(
                        i,
                        next.0 - previous.0,
                        next.1 - previous.1,
                    );
                    self.control_drag = Some(next);
                }
            }
            "down" => {
                ensure!(
                    !secondary && self.tool == Tool::Select,
                    "Drag requires primary Select tool"
                );
                self.selected = canvas::hit(
                    &Scene::from_document(self.doc.as_ref().unwrap()),
                    &self.view,
                    at,
                );
                if self.selected.is_some() {
                    self.checkpoint();
                    self.control_drag = Some(self.view.snap(at));
                }
            }
            "up" => self.control_drag = None,
            "click" => {
                if secondary {
                    self.tool = Tool::Select;
                    self.wire_start = None;
                } else {
                    self.canvas_click(
                        at,
                        p.get("double").and_then(Value::as_bool).unwrap_or(false),
                    );
                }
            }
            _ => anyhow::bail!("Unknown pointer phase"),
        }
        journal::record(
            &before,
            self.doc.as_ref().unwrap(),
            p.get("actor")
                .and_then(Value::as_str)
                .unwrap_or("external:pointer"),
            "pointer",
        )?;
        self.observed_doc = self.doc.clone();
        Ok(
            json!({"revision":journal::revision(self.doc.as_ref().unwrap()),"tool":self.tool.name(),"cursor_mm":self.cursor}),
        )
    }
    pub(super) fn flush_human_history(&mut self) {
        if let Some((before, after, _)) = self.pending_human.take()
            && let Err(e) = journal::record(&before, &after, "user", "edit")
        {
            self.status = format!("History could not be written: {e}");
        }
    }
    pub(super) fn observe_history(&mut self) {
        if let (Some(before), Some(after)) = (&self.observed_doc, &self.doc)
            && before.root_uuid() == after.root_uuid()
            && before.root != after.root
        {
            let first = self
                .pending_human
                .as_ref()
                .map(|(d, _, _)| d.clone())
                .unwrap_or_else(|| before.clone());
            self.pending_human = Some((first, after.clone(), journal::now()));
        }
        self.observed_doc = self.doc.clone();
        if let Some((_, _, time)) = &self.pending_human
            && journal::now() - time > 500
        {
            self.flush_human_history();
        }
    }
}
impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Symbol => "symbol",
            Self::Wire => "wire",
            Self::Junction => "junction",
            Self::Label => "label",
            Self::Text => "text",
            Self::NoConnect => "no_connect",
            Self::Move => "move",
        }
    }
    fn parse(name: &str) -> Option<Self> {
        [
            Self::Select,
            Self::Symbol,
            Self::Wire,
            Self::Junction,
            Self::Label,
            Self::Text,
            Self::NoConnect,
            Self::Move,
        ]
        .into_iter()
        .find(|t| t.name() == name)
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.observe_history();
        self.flush_human_history();
    }
}
