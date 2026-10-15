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
        if !self.doc.is_some() || !self.save(false) {
            return;
        }
        let doc = self.doc.as_ref().unwrap().clone();
        if !self.confirm_export_delete(&doc) {
            return;
        }
        match self.control_request("delete_project", &serde_json::json!({"actor":"user"})) {
            Ok(result) => self.status = format!("Archived · {}", result["recovery_folder"]),
            Err(e) => self.fail(e),
        }
    }
    pub(super) fn confirm_export_delete(&mut self, doc: &Document) -> bool {
        let choice=rfd::MessageDialog::new().set_title("Export before deleting project?").set_description("Yes: export a native schematic, PDF and Excel BOM first, then archive. No: archive without exporting. Cancel: keep the project. Archived files remain recoverable.").set_buttons(rfd::MessageButtons::YesNoCancel).show();
        if choice == rfd::MessageDialogResult::Cancel {
            return false;
        }
        if choice == rfd::MessageDialogResult::Yes {
            let Some(folder) = rfd::FileDialog::new()
                .set_title("Choose export destination outside the project")
                .pick_folder()
            else {
                return false;
            };
            let result = (|| -> anyhow::Result<()> {
                let source = doc
                    .path
                    .as_ref()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .canonicalize()?;
                let dest = folder.canonicalize()?;
                anyhow::ensure!(
                    !dest.starts_with(&source),
                    "Choose an export folder outside the project so deletion does not archive your export"
                );
                let out = dest.join(format!(
                    "{}-export-{}",
                    doc.project_name(),
                    jenga_tools::journal::now()
                ));
                std::fs::create_dir(&out)?;
                export::schematic(doc, &out.join("schematic.kicad_sch"))?;
                export::pdf(doc, &out.join("schematic.pdf"))?;
                export::bom_xlsx(doc, &out.join("bom.xlsx"))?;
                Ok(())
            })();
            if let Err(e) = result {
                self.fail(format!("Export failed; project retained: {e}"));
                return false;
            }
        }
        true
    }
}
