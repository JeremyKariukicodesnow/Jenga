use super::*;
impl App {
    pub(super) fn asset_import_dialog(&mut self, personal: bool) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import symbol, footprint or 3D model")
            .add_filter(
                "Electronic design assets",
                &[
                    "kicad_sym",
                    "kicad_mod",
                    "step",
                    "stp",
                    "wrl",
                    "glb",
                    "stl",
                    "obj",
                ],
            )
            .pick_file()
        else {
            return;
        };
        let name: String = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        match self.control_request("import_asset",&serde_json::json!({"path":path,"name":if name.is_empty(){"Imported"}else{&name},"scope":if personal{"personal"}else{"project"},"actor":"user"})) {Ok(v)=>{self.status=format!("Imported asset: {}",v["path"]);self.reload_libraries();self.project_previews.clear();},Err(e)=>self.fail(e)}
    }
    pub(super) fn copy_clipboard(&mut self, ctx: &egui::Context) -> bool {
        if self.pcb.active {
            let result = (|| -> anyhow::Result<String> {
                let b = self
                    .pcb
                    .board
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Open a PCB first"))?;
                let ids = if self.copy_all {
                    None
                } else {
                    self.pcb
                        .selected
                        .and_then(|i| b.root.items().get(i))
                        .map(|n| vec![jenga_core::pcb::id(n).to_owned()])
                };
                anyhow::ensure!(
                    self.copy_all || ids.is_some(),
                    "Select a PCB object or Ctrl+A to copy all"
                );
                jenga_core::pcb_clipboard::copy(b, ids.as_deref())
            })();
            return match result {
                Ok(text) => {
                    ctx.copy_text(text);
                    self.status = "Copied native PCB objects for Jenga/KiCad".into();
                    true
                }
                Err(e) => {
                    self.fail(e);
                    false
                }
            };
        }
        let Some(doc) = &self.doc else {
            return false;
        };
        let ids = if self.copy_all {
            None
        } else if !self.copy_selection.is_empty() {
            Some(self.copy_selection.clone())
        } else {
            self.selected.and_then(|i| {
                doc.root
                    .items()
                    .get(i)?
                    .child("uuid")
                    .map(|n| vec![n.value(1).to_owned()])
            })
        };
        if !self.copy_all && ids.is_none() {
            self.status = "Select an object, or Ctrl+A to copy the entire schematic".into();
            return false;
        }
        match jenga_core::clipboard::copy(doc, ids.as_deref()) {
            Ok(text) => {
                ctx.copy_text(text);
                self.status = "Copied native schematic objects · paste into Jenga or KiCad".into();
                true
            }
            Err(e) => {
                self.fail(e);
                false
            }
        }
    }
    pub(super) fn cut_clipboard(&mut self, ctx: &egui::Context) {
        if self.pcb.active {
            if self.copy_clipboard(ctx) {
                if let Some(b) = self.pcb.board.as_mut() {
                    b.undo.push(b.state());
                    let selected = self.pcb.selected;
                    let all = self.copy_all;
                    let mut i = 0;
                    b.root.items_mut().retain(|n| {
                        let keep = if all {
                            !matches!(
                                n.name(),
                                "footprint"
                                    | "segment"
                                    | "via"
                                    | "zone"
                                    | "gr_text"
                                    | "gr_line"
                                    | "gr_arc"
                                    | "gr_rect"
                                    | "gr_circle"
                                    | "gr_poly"
                                    | "group"
                            )
                        } else {
                            Some(i) != selected
                        };
                        i += 1;
                        keep
                    });
                    b.dirty = true;
                    b.redo.clear();
                }
                self.copy_all = false;
                self.pcb.selected = None;
            }
            return;
        }
        if !self.copy_clipboard(ctx) {
            return;
        }
        self.checkpoint();
        if self.copy_all {
            self.doc
                .as_mut()
                .unwrap()
                .root
                .items_mut()
                .retain(|n| !editable(n.name()));
            self.doc.as_mut().unwrap().dirty = true;
        } else if !self.copy_selection.is_empty() {
            let ids = self.copy_selection.clone();
            self.doc.as_mut().unwrap().root.items_mut().retain(|n| {
                !n.child("uuid")
                    .is_some_and(|n| ids.iter().any(|s| s == n.value(1)))
            });
            self.doc.as_mut().unwrap().dirty = true;
            self.selected = None;
        } else if let Some(i) = self.selected.take() {
            self.doc.as_mut().unwrap().remove(i);
        }
        self.copy_all = false;
        self.copy_selection.clear();
    }
    pub(super) fn paste_clipboard(&mut self) {
        if self.pcb.active {
            let result = (|| -> anyhow::Result<serde_json::Value> {
                let text = arboard::Clipboard::new()?.get_text()?;
                let revision = self
                    .pcb
                    .board
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Open a PCB first"))?
                    .revision();
                let mut params = serde_json::json!({"text":text,"at":self.cursor,"target":"pcb","expected_revision":revision,"actor":"user","dry_run":true});
                let preview = self.control_request("paste_objects", &params)?;
                if preview["joined_nets"]
                    .as_array()
                    .is_some_and(|n| !n.is_empty())
                {
                    if rfd::MessageDialog::new()
                        .set_title("Join named PCB nets?")
                        .set_description(format!(
                            "Pasting joins these existing nets: {}. Continue?",
                            preview["joined_nets"]
                        ))
                        .set_buttons(rfd::MessageButtons::OkCancel)
                        .show()
                        != rfd::MessageDialogResult::Ok
                    {
                        return Ok(serde_json::Value::Null);
                    }
                    params["allow_named_net_merges"] = serde_json::json!(true);
                }
                params["dry_run"] = serde_json::json!(false);
                self.control_request("paste_objects", &params)
            })();
            match result {
                Ok(_) => {
                    self.copy_all = false;
                    self.pcb.selected = None;
                    self.status = "Pasted native PCB objects; run DRC before saving".into();
                }
                Err(e) => self.fail(e),
            }
            return;
        }
        let result = (|| -> anyhow::Result<serde_json::Value> {
            let mut clipboard = arboard::Clipboard::new()?;
            let text = clipboard.get_text()?;
            let doc = self
                .doc
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Open a schematic first"))?;
            let revision = jenga_tools::journal::revision(doc);
            self.control_request("paste_objects",&serde_json::json!({"text":text,"at":self.cursor,"expected_revision":revision,"actor":"user"}))
        })();
        match result {
            Ok(_) => {
                self.copy_all = false;
                self.copy_selection.clear();
                self.status = "Pasted at cursor · Undo available".into();
            }
            Err(e) => self.fail(e),
        }
    }
    pub(super) fn transfer_dialog(&mut self, method: &str) {
        let result = (|| -> anyhow::Result<serde_json::Value> {
            let mut p = serde_json::json!({"actor":"user"});
            if method != "move_project" {
                let Some(path) = rfd::FileDialog::new()
                    .set_title(if method == "import_project" {
                        "Import KiCad project"
                    } else {
                        "Import independent schematic or PCB"
                    })
                    .add_filter(
                        "Native KiCad",
                        if method == "import_project" {
                            &["kicad_pro"][..]
                        } else {
                            &["kicad_sch", "kicad_pcb"][..]
                        },
                    )
                    .pick_file()
                else {
                    return Ok(serde_json::Value::Null);
                };
                p["path"] = serde_json::json!(path);
            }
            if method != "import_document" {
                let Some(folder) = rfd::FileDialog::new()
                    .set_title("Choose destination parent folder")
                    .pick_folder()
                else {
                    return Ok(serde_json::Value::Null);
                };
                p["folder"] = serde_json::json!(folder);
            }
            self.control_request(method, &p)
        })();
        match result {
            Ok(v) if !v.is_null() => {
                let path = v["path"].as_str().map(PathBuf::from);
                if let Some(path) = path {
                    self.workspace_ui.root = Some(path.clone());
                    self.workspace_ui.show = true;
                    self.workspace_ui.reset_scroll = true;
                    self.settings.remember(path);
                    self.persist();
                }
                self.project_previews.clear();
                self.status = format!("{method} completed; original data preserved or recoverable");
            }
            Ok(_) => {}
            Err(e) => self.fail(e),
        }
    }
    pub(super) fn delete_current_project(&mut self) {
        if self.pcb.active {
            if rfd::MessageDialog::new().set_title("Archive PCB project?").set_description("Save and move this project into recovery storage? It can be restored from Archived projects.").set_buttons(rfd::MessageButtons::OkCancel).show()==rfd::MessageDialogResult::Ok&&self.save_pcb(){if let Err(e)=self.control_request("pcb_archive_project",&serde_json::json!({"actor":"user"})){self.fail(e);}self.workspace_ui.show=false;}
            return;
        }
        self.pending_delete = self.doc.clone();
    }
    pub(super) fn delete_dialog(&mut self, ctx: &egui::Context) {
        let Some(doc) = self.pending_delete.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Delete project?").open(&mut open).collapsible(false).resizable(false).show(ctx,|ui|{
            ui.heading(doc.project_name());ui.label("Files are moved to Jenga recovery storage. Exporting is optional.");
            if doc.dirty{ui.colored_label(ACCENT,"Unsaved edits will be discarded by Delete. Save and delete preserves them first.");}
            ui.horizontal(|ui|{
                for(label,save)in[("Delete",false),("Save and delete",true)]{
                    if ui.button(label).clicked(){
                        if save&&self.doc.as_ref().and_then(|d|d.path.as_ref())==doc.path.as_ref()&&!self.save(false){return;}
                        let mut saved=self.doc.as_ref().filter(|d|d.path==doc.path).cloned().unwrap_or(doc.clone());saved.dirty=false;
                        match jenga_tools::projects::archive_project(&saved){Ok(result)=>{let _=jenga_tools::catalog::record("delete_project",saved.path.as_deref(),None);self.settings.recent.retain(|p|Some(p)!=saved.path.as_ref());if self.doc.as_ref().and_then(|d|d.path.as_ref())==saved.path.as_ref(){self.doc=None;self.workspace_ui.show=false;}self.pending_delete=None;self.persist();self.status=format!("Deleted · recoverable at {}",result["recovery_folder"]);},Err(e)=>self.fail(e)}
                    }
                }
                if ui.button("Cancel").clicked(){self.pending_delete=None;}
            });
        });
        if !open {
            self.pending_delete = None;
        }
    }
    pub(super) fn confirm_export_delete(&mut self, doc: &Document) -> bool {
        self.pending_delete = Some(doc.clone());
        false
    }
}
