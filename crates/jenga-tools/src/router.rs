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
        if matches!(method, "view_skill" | "export_skill") {
            return self.engine.request(method, p);
        }
        if self.mode == "headless" {
            return self.engine.request(method, p);
        }
        if let Some(e) = &self.endpoint {
            // Only a failed TCP connection confirms closure. A hung UI is not closure.
            if let Ok(address) = e.address.parse::<std::net::SocketAddr>()
                && std::net::TcpStream::connect_timeout(
                    &address,
                    std::time::Duration::from_millis(200),
                )
                .is_err()
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
                }
                self.endpoint = None;
            }
        }
        if self.endpoint.is_none() {
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
            let result = transport::call(e, method, p)?;
            // Read-only observation after success; never retry the operation if this fails.
            if let Ok(state) = transport::call(e, "ui_state", &json!({})) {
                let doc = &state["document"];
                self.live_path = doc["file"].as_str().map(PathBuf::from);
                self.live_dirty = doc["dirty"].as_bool().unwrap_or(false);
            } else {
                self.live_dirty = true;
            }
            Ok(result)
        } else {
            self.engine.request(method, p)
        }
    }
}
