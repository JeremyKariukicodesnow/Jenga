use super::*;
impl App {
    pub(super) fn copy_clipboard(&mut self, ctx: &egui::Context) -> bool {
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
