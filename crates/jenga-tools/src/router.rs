//! One persistent tool connection. Never replay a failed mutation on another backend.
use crate::{engine::Engine, transport};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::PathBuf;
pub struct Router {
    pub engine: Engine,
    endpoint: Option<transport::Endpoint>,
    live_path: Option<PathBuf>,
    live_dirty: bool,
    mode: String,
}
impl Router {
    pub fn new(workspace: PathBuf, mode: &str) -> Self {
        Self {
            engine: Engine::new(workspace),
            endpoint: None,
            live_path: None,
            live_dirty: false,
            mode: mode.into(),
        }
    }
    pub fn request(&mut self, method: &str, p: &Value) -> Result<Value> {
        if matches!(
            method,
            "view_skill"
                | "export_skill"
                | "help"
                | "operation_status"
                | "operation_cancel"
                | "operations"
                | "export_formats"
        ) {
            return self.engine.request(method, p);
        }
        if self.mode == "headless" {
            return self.engine.request(method, p);
        }
        if let Some(e) = &self.endpoint {
            // Confirm process exit or failed TCP connection; a blocked UI is not closure.
            if transport::process_alive(e.pid) == Some(false)
                || (if let Ok(address) = e.address.parse::<std::net::SocketAddr>() {
                    std::net::TcpStream::connect_timeout(
                        &address,
                        std::time::Duration::from_millis(200),
                    )
                    .is_err()
                } else {
                    false
                })
            {
                ensure!(
                    self.mode == "auto",
                    "The selected desktop session closed; start it again or use automatic mode"
                );
                ensure!(
                    !self.live_dirty,
                    "Desktop closed with unsaved edits. Reopen/recover the document; no disk fallback was attempted."
                );
                if let Some(path) = &self.live_path {
                    self.engine.request("open", &json!({"path":path}))?;
                } else {
                    self.engine.doc = None;
                    self.engine.board = None;
                }
                self.endpoint = None;
            }
        }
        if self.endpoint.is_none() {
            if self.engine.board.as_ref().is_some_and(|b| b.dirty)
                && !matches!(method, "open" | "pcb_open")
            {
                return self.engine.request(method, p);
            }
            if let Ok(e) = transport::connect(if self.mode == "auto" || self.mode == "live" {
                "auto"
            } else {
                &self.mode
            }) {
                let state = transport::call(&e, "ui_state", &json!({}))?;
                let desktop = state.get("document").unwrap_or(&Value::Null);
                if let Some(doc) = &self.engine.doc {
                    // Finish outstanding headless edits before handing the file to a window.
                    if doc.dirty {
                        return self.engine.request(method, p);
                    }
                    if !matches!(
                        method,
                        "open"
                            | "create_project"
                            | "new"
                            | "create_schematic"
                            | "help"
                            | "list_projects"
                    ) && let Some(path) = &doc.path
                    {
                        let live = desktop
                            .get("file")
                            .and_then(Value::as_str)
                            .map(PathBuf::from);
                        ensure!(
                            live.as_ref().is_none_or(|other| other == path),
                            "Jenga has another project open. Call open explicitly to choose the intended project."
                        );
                        if live.is_none() {
                            transport::call(&e, "open", &json!({"path":path}))?;
                        }
                    }
                }
                self.endpoint = Some(e);
            } else if self.mode != "auto" {
                anyhow::bail!("No responding Jenga window for this connection");
            }
        }
        if let Some(e) = &self.endpoint {
            if p["background"] == true
                && matches!(
                    method,
                    "pcb_preview"
                        | "pcb_autoroute"
                        | "pcb_autoplace"
                        | "build_pcb_from_schematic"
                        | "pcb_verify"
                        | "pcb_3d"
                        | "pcb_export"
                        | "pcb_export_3d_view"
                        | "preview_edit"
                )
            {
                // The assistant's worker owns the calculation, so closing the
                // desktop does not kill it. Only immutable snapshots cross IPC.
                if let Ok(snapshot) = transport::call(e, "engineering_snapshot", &json!({})) {
                    self.live_dirty =
                        snapshot["document"]["dirty"] == true || snapshot["board"]["dirty"] == true;
                    self.live_path = snapshot[if snapshot["active_kind"] == "pcb" {
                        "board"
                    } else {
                        "document"
                    }]["file"]
                        .as_str()
                        .map(PathBuf::from);
                    let mut worker = Engine::new(self.engine.workspace.clone());
                    if let Some(source) = snapshot["document"]["source"].as_str() {
                        let path = snapshot["document"]["file"].as_str().map(PathBuf::from);
                        let mut d = path
                            .as_ref()
                            .filter(|p| p.is_file())
                            .map(|p| jenga_core::document::Document::open(p))
                            .transpose()?
                            .unwrap_or_else(jenga_core::document::Document::new);
                        d.root = jenga_core::sexpr::parse(source)?;
                        d.path = path;
                        d.dirty = snapshot["document"]["dirty"] == true;
                        worker.doc = Some(d);
                    }
                    if let Some(source) = snapshot["board"]["source"].as_str() {
                        let path = snapshot["board"]["file"].as_str().map(PathBuf::from);
                        let mut b = if let Some(p) = path.as_ref().filter(|p| p.is_file()) {
                            jenga_core::pcb::Board::open(p)?
                        } else {
                            jenga_core::pcb::Board::new(80., 60.)?
                        };
                        b.root = jenga_core::sexpr::parse(source)?;
                        b.path = path;
                        b.dirty = snapshot["board"]["dirty"] == true;
                        b.rules = serde_json::from_value(snapshot["board"]["rules"].clone())?;
                        b.schematic = snapshot["board"]["schematic"].as_str().map(PathBuf::from);
                        b.extra_nets =
                            serde_json::from_value(snapshot["board"]["extra_nets"].clone())?;
                        worker.board = Some(b);
                    }
                    worker.active_kind = snapshot["active_kind"]
                        .as_str()
                        .unwrap_or("schematic")
                        .into();
                    let mut result = worker.request(method, p)?;
                    result["source_snapshot_age_ms"] = snapshot["snapshot_age_ms"].clone();
                    result["worker_owner"] = json!("assistant_connection");
                    return Ok(result);
                }
            }
            let result = transport::call(e, method, p)?;
            // Read-only observation after success; never retry the operation if this fails.
            if let Ok(state) = transport::call(e, "ui_state", &json!({})) {
                let doc = if state["active_editor"] == "pcb" {
                    &state["pcb"]
                } else {
                    &state["document"]
                };
                self.live_path = doc["file"].as_str().map(PathBuf::from);
                self.live_dirty = state["document"]["dirty"].as_bool().unwrap_or(false)
                    || state["pcb"]["dirty"].as_bool().unwrap_or(false);
            } else {
                self.live_dirty = true;
            }
            Ok(result)
        } else {
            self.engine.request(method, p)
        }
    }
}
