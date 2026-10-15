//! External tool control. There is deliberately no AI service or chat interface.
use super::*;
use anyhow::{Context, Result, ensure};
use jenga_tools::{engine, journal};
use serde_json::{Value, json};
impl App {
    pub(super) fn poll_project_changes(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(std::time::Duration::from_millis(750));
        let now = journal::now();
        if now < self.sync_at {
            return;
        }
        self.sync_at = now + 750;
        let events: Vec<_> = jenga_tools::catalog::events()
            .into_iter()
            .filter(|e| self.seen_project_events.insert(e.id.clone()))
            .collect();
        if !events.is_empty() {
            for event in &events {
                if let Some(active) = self.doc.as_ref().and_then(|d| d.path.clone()) {
                    if !active.is_file()
                        && matches!(event.kind.as_str(), "rename" | "delete_project")
                        && event
                            .old
                            .as_ref()
                            .and_then(|p| p.parent())
                            .is_some_and(|folder| active.starts_with(folder))
                    {
                        if self.doc.as_ref().is_some_and(|d| d.dirty) {
                            self.external_conflict = true;
                            self.status="Project moved externally; unsaved edits are preserved. Save a separate copy.".into();
                        } else if event.kind == "delete_project" {
                            self.doc = None;
                            self.workspace_ui.show = false;
                        } else if let (Some(old), Some(new)) = (&event.old, &event.path) {
                            if let (Some(a), Some(b)) = (old.parent(), new.parent()) {
                                if let Ok(relative) = active.strip_prefix(a) {
                                    if let Ok(updated) = Document::open(&b.join(relative)) {
                                        self.set_doc(updated);
                                    }
                                }
                            }
                        }
                    }
                }
                if matches!(event.kind.as_str(), "rename" | "delete_project")
                    && self
                        .doc
                        .as_ref()
                        .and_then(|d| d.path.as_ref())
                        .is_some_and(|p| jenga_tools::catalog::affects_active(event, p))
                {
                    if self.doc.as_ref().is_some_and(|d| d.dirty) {
                        self.external_conflict = true;
                        self.status =
                            "Project moved externally; save your edits as a separate copy".into();
                    } else if let Some(path) = &event.path {
                        if let Ok(doc) = Document::open(path) {
                            self.set_doc(doc);
                        }
                    } else {
                        self.doc = None;
                        self.selected = None;
                        self.history.clear();
                        self.redo.clear();
                    }
                }
            }
            let before = self.settings.recent.clone();
            jenga_tools::catalog::apply(&mut self.settings.recent, &events);
            self.project_previews.clear();
            if before != self.settings.recent {
                self.persist();
            }
            self.publish_control_snapshot();
        }
        if let Some(board) = &self.pcb.board
            && board.disk_changed().unwrap_or(false)
        {
            if board.dirty {
                self.status="PCB changed externally; your unsaved edits are preserved. Save a separate copy or reopen.".into();
            } else if let Some(path) = board.path.clone() {
                match jenga_core::pcb::Board::open(&path) {
                    Ok(b) => {
                        self.pcb.board = Some(b);
                        self.pcb.fit = true;
                        self.status = "Reloaded external PCB changes".into();
                    }
                    Err(e) => self.fail(e),
                }
            }
        }
        if let Some(doc) = &self.doc
            && doc.disk_changed().unwrap_or(false)
        {
            if doc.dirty {
                if !self.external_conflict {
                    self.status="External file change detected. Your unsaved edits are preserved; save a separate copy or reopen to reload.".into();
                    self.toast_until = now + 7000;
                }
                self.external_conflict = true;
            } else if let Some(path) = doc.path.clone()
                && path.is_file()
            {
                if let Ok(updated) = Document::open(&path) {
                    self.set_doc(updated);
                    self.status = "Reloaded external changes".into();
                    self.toast_until = now + 3000;
                    self.publish_control_snapshot();
                }
            }
        }
    }
    pub(super) fn publish_control_snapshot(&self) {
        if let Some(server) = &self.control {
            server.publish_board(self.doc.clone(),self.pcb.board.clone(),json!({"running_app":true,"pid":std::process::id(),"version":env!("CARGO_PKG_VERSION"),"indexing":self.library_rx.is_some(),"tool":self.tool.name(),"cursor_mm":self.cursor,"selection":self.selected.and_then(|i|self.doc.as_ref()?.root.items().get(i)?.child("uuid")).map(|n|n.value(1)),"view":{"center":self.view.center,"scale":self.view.scale,"grid":self.view.grid,"show_grid":self.view.show_grid,"canvas_pixels":[self.canvas_rect.min.x,self.canvas_rect.min.y,self.canvas_rect.width(),self.canvas_rect.height()]},"dialogs":{"symbol_editor":self.symbol_editor.is_some(),"project_dashboard":self.workspace_ui.show,"connect_assistant":self.connect_dialog,"help":self.help_dialog,"chooser":self.chooser,"page":self.page_dialog,"libraries":self.library_dialog,"erc":self.show_checks},"pcb":self.pcb.board.as_ref().map(|b|b.inspect(&json!({"include_pads":false}))),"active_editor":if self.pcb.active{"pcb"}else{"schematic"},"pcb_pending":self.pcb.pending.is_some(),"status":self.status}));
        }
    }
    pub(super) fn poll_control(&mut self, ctx: &egui::Context) {
        if self
            .control
            .as_ref()
            .is_some_and(|s| s.snapshot_age() > 750)
        {
            self.publish_control_snapshot();
        }
        let mut jobs = vec![];
        if let Some(server) = &self.control {
            while let Ok(job) = server.jobs.try_recv() {
                jobs.push(job);
            }
        }
        for job in jobs {
            if journal::now() >= job.expires_ms
                || job
                    .started
                    .compare_exchange(
                        0,
                        1,
                        std::sync::atomic::Ordering::AcqRel,
                        std::sync::atomic::Ordering::Acquire,
                    )
                    .is_err()
            {
                job.started.store(2, std::sync::atomic::Ordering::Release);
                let id = job.request["params"]["operation_id"].as_str().unwrap_or("");
                let response = if id.is_empty() {
                    json!({"error":"Expired request cancelled before execution"})
                } else {
                    jenga_tools::operations::reject(
                        id,
                        "Expired request cancelled before execution; no edit accepted",
                    )
                };
                let _ = job.reply.send(response);
                continue;
            }
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
            if jenga_tools::pcb_tools::METHODS.contains(&method) {
                let result = self.start_pcb_worker(method, &p, ctx);
                let _ = job.reply.send(match result {
                    Ok(v) => json!({"result":v}),
                    Err(e) => json!({"error":format!("{e:#}")} ),
                });
                continue;
            }
            let result = self.control_request(method, &p);
            if let Err(e) = &result {
                if let Some(id) = p["operation_id"].as_str() {
                    if jenga_tools::operations::status(id).is_ok_and(|r| r["status"] == "pending") {
                        let _ = jenga_tools::operations::finish(id, &Err(anyhow::anyhow!("{e:#}")));
                    }
                }
            }
            self.publish_control_snapshot();
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
                json!({"running_app":true,"pid":std::process::id(),"version":env!("CARGO_PKG_VERSION"),"document":self.doc.as_ref().map(engine::inspect),"selection":self.selected.and_then(|i|self.doc.as_ref()?.root.items().get(i)?.child("uuid")).map(|n|n.value(1)),"tool":self.tool.name(),"cursor_mm":self.cursor,"view":{"center":self.view.center,"scale":self.view.scale,"grid":self.view.grid,"show_grid":self.view.show_grid,"canvas_pixels":[self.canvas_rect.min.x,self.canvas_rect.min.y,self.canvas_rect.width(),self.canvas_rect.height()]},"dialogs":{"symbol_editor":self.symbol_editor.is_some(),"project_dashboard":self.workspace_ui.show,"connect_assistant":self.connect_dialog,"help":self.help_dialog,"chooser":self.chooser,"page":self.page_dialog,"history":self.history_dialog,"rename":self.rename_dialog,"text":self.text_dialog,"libraries":self.library_dialog,"erc":self.show_checks},"indexing":self.library_rx.is_some(),"pcb":self.pcb.board.as_ref().map(|b|b.inspect(&json!({"include_pads":false}))),"active_editor":if self.pcb.active{"pcb"}else{"schematic"},"pcb_pending":self.pcb.pending.is_some(),"status":self.status}),
            );
        }
        if method == "ui_action" {
            match engine::string(p, "action")? {
                "dismiss_dialogs" => {
                    self.pcb.show_3d = false;
                    self.pcb.preview_dialog = false;
                    self.dismiss_footprints();
                    self.symbol_editor = None;
                    self.symbol_browser.open = false;
                    self.pending_delete = None;
                    self.workspace_ui.page_dialog = false;
                    self.workspace_ui.manager = false;
                    self.workspace_ui.rename_path = None;
                    self.workspace_ui.export_dialog = false;
                    self.connect_dialog = false;
                    self.help_dialog = false;
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
                "show_connect_assistant" => {
                    self.connect_dialog = true;
                }
                "show_help" => {
                    self.help_dialog = true;
                }
                "show_chooser" => self.choose_tool(Tool::Symbol),
                "show_page_settings" => self.page_dialog = true,
                "show_project_dashboard" => {
                    self.workspace_ui.show = true;
                    self.workspace_ui.reset_scroll = true;
                }
                "show_footprint_browser" => self.footprints.open = true,
                "show_pcb_editor" => {
                    ensure!(self.pcb.board.is_some(), "No PCB open");
                    self.pcb.active = true;
                    self.workspace_ui.show = false;
                }
                "show_3d" => {
                    self.pcb.show_3d = true;
                }
                "show_archived_projects" => self.archived_open = true,
                "show_editor" => self.workspace_ui.show = false,
                "show_schematic_editor" => {
                    ensure!(self.doc.is_some(), "No schematic open");
                    self.pcb.active = false;
                    self.workspace_ui.show = false;
                }
                "fullscreen_3d" => {
                    self.pcb.show_3d = true;
                    self.pcb.fullscreen_3d = true;
                }
                "show_page_manager" => self.workspace_ui.manager = true,
                "show_symbol_browser" => {
                    self.symbol_browser.open = true;
                    self.symbol_browser.project_only = true;
                }
                "show_delete_confirmation" => self.delete_current_project(),
                "show_symbol_editor" => self.symbol_editor = Some(Default::default()),
                "show_page_export" => {
                    self.workspace_ui.export_dialog = true;
                    self.workspace_ui.selected_pages = self
                        .doc
                        .as_ref()
                        .and_then(|d| d.path.clone())
                        .into_iter()
                        .collect();
                }
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
            !matches!(method, "edit" | "preview_edit" | "connect_block")
                || self.library_rx.is_none(),
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
        engine.board = self.pcb.board.clone();
        engine.active_kind = if self.pcb.active { "pcb" } else { "schematic" }.into();
        engine.libraries = self.libraries.take();
        let before = self.doc.clone();
        let result = engine.request(method, p);
        self.libraries = engine.libraries;
        if result.is_ok() {
            if let Some(before) = before
                && matches!(
                    method,
                    "edit" | "paste_objects" | "connect_block" | "commit_preview"
                )
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
            self.pcb.active = engine.active_kind == "pcb";
            self.pcb.board = engine.board;
            if (method.starts_with("pcb_") || method == "build_pcb_from_schematic")
                && self.pcb.board.is_some()
            {
                self.pcb.active = true;
                self.pcb.fit = true;
                self.workspace_ui.show = false;
            }
            if matches!(
                method,
                "open"
                    | "create_project"
                    | "create_schematic"
                    | "rename"
                    | "save_project_as"
                    | "delete_project"
                    | "delete_page"
            ) {
                self.workspace_ui.root = self.doc.as_ref().and_then(|d| d.path.clone());
                self.workspace_ui.reset_scroll = true;
            }
            self.observed_doc = self.doc.clone();
            if matches!(
                method,
                "new"
                    | "open"
                    | "create_project"
                    | "create_schematic"
                    | "rename"
                    | "close"
                    | "delete_project"
                    | "delete_page"
                    | "rename_page"
                    | "reorder_pages"
                    | "save_project_as"
            ) {
                self.selected = None;
                self.copy_all = false;
                self.copy_selection.clear();
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
                "import_library"
                    | "write_library"
                    | "create_library"
                    | "save_symbol"
                    | "copy_symbol"
                    | "manage_symbol"
                    | "footprint_save"
                    | "footprint_manage"
                    | "footprint_copy"
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
                && !self.connect_dialog
                && !self.help_dialog
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
