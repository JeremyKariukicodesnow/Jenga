use super::*;
impl App {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.support_dialogs(ctx);
        self.workspace_dialogs(ctx);
        if self.new_project {
            let mut open = true;
            egui::Window::new("Create project")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Project name");
                    ui.text_edit_singleline(&mut self.project_name);
                    ui.add_space(8.);
                    ui.label("Parent folder");
                    ui.horizontal(|ui| {
                        ui.label(self.project_folder.display().to_string());
                        if ui.button("Choose…").clicked()
                            && let Some(folder) = rfd::FileDialog::new()
                                .set_directory(&self.project_folder)
                                .pick_folder()
                        {
                            self.project_folder = folder;
                        }
                    });
                    ui.add_space(12.);
                    if ui.button("Create project and schematic").clicked() {
                        match create_project(&self.project_folder, &self.project_name) {
                            Ok(doc) => {
                                self.set_doc(doc);
                                self.new_project = false;
                            }
                            Err(e) => self.fail(e),
                        }
                    }
                });
            self.new_project &= open;
        }
        if self.page_dialog {
            let mut open = true;
            let mut finish = false;
            egui::Window::new("Page settings · title block").open(&mut open).default_width(410.).show(ctx,|ui| {
                if let Some(doc)=&self.doc {
                    let mut block=jenga_core::page::TitleBlock::read(doc);let original=block.clone();
                    ui.label("Edit this page’s native KiCad worksheet. Imported metadata is retained.");ui.label(format!("Page file: {}",doc.path.as_ref().map(|p|p.display().to_string()).unwrap_or_default()));
                    for (label,value) in [("Title",&mut block.title),("Company",&mut block.company),("Date",&mut block.date),("Revision",&mut block.revision)] {ui.label(label);ui.text_edit_singleline(value);}
                    egui::CollapsingHeader::new("Comments (1–9)").show(ui,|ui|{for(i,value)in block.comments.iter_mut().enumerate(){ui.label(format!("Comment {}",i+1));ui.text_edit_singleline(value);}});
                    let mut paper=doc.root.child("paper").map(|n|n.value(1)).unwrap_or("A4").to_owned();let before=paper.clone();
                    egui::ComboBox::from_id_salt("paper_size").selected_text(&paper).show_ui(ui,|ui|{for size in ["A0","A1","A2","A3","A4","A5","USLetter","USLegal"]{ui.selectable_value(&mut paper,size.into(),size);}});
                    if block!=original || paper!=before {self.checkpoint();let doc=self.doc.as_mut().unwrap();block.write(doc);if paper!=before{doc.root.set_child(Node::list("paper",vec![Node::string(paper)]));self.fit=true;}}
                    ui.horizontal(|ui|{if ui.button("Save and close").clicked()&&self.save(false){finish=true;}if ui.button("Close (keep unsaved edits)").clicked(){finish=true;}});
                    ui.label(RichText::new("Default worksheet borders are rendered. Custom .kicad_wks templates are retained in projects but are not rendered yet.").small().weak());
                }
            });
            self.page_dialog = open && !finish;
        }
        if self.rename_dialog {
            let mut open = true;
            egui::Window::new("Rename schematic / project").open(&mut open).default_width(380.).show(ctx,|ui|{
                ui.label("New name");ui.text_edit_singleline(&mut self.rename_name);ui.checkbox(&mut self.rename_project,"Rename native project and its root schematic");
                if self.rename_project{ui.checkbox(&mut self.rename_folder,"Also rename the project folder");}
                ui.label(RichText::new("Existing destinations are never overwritten. Native references and UUIDs are retained.").small().weak());
                if ui.button("Rename").clicked(){let params=serde_json::json!({"name":self.rename_name,"kind":if self.rename_project{"project"}else{"schematic"},"move_folder":self.rename_project&&self.rename_folder,"actor":"user"});
                    match self.control_request("rename",&params){Ok(_)=>{self.rename_dialog=false;self.status=format!("Renamed to {}",self.rename_name);},Err(e)=>self.fail(e)}
                }
            });
            self.rename_dialog &= open;
        }
        if self.history_dialog {
            let mut open = true;
            egui::Window::new("Change history")
                .open(&mut open)
                .default_size([650., 380.])
                .show(ctx, |ui| {
                    ui.label(
                        "User and external-tool changes are attributed in the project journal.",
                    );
                    if let Some(doc) = &self.doc {
                        match jenga_tools::journal::read(doc, 100) {
                            Ok(entries) => {
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    for entry in entries.into_iter().rev() {
                                        ui.horizontal(|ui| {
                                            ui.strong(entry["actor"].as_str().unwrap_or("unknown"));
                                            ui.label(entry["operation"].as_str().unwrap_or("edit"));
                                            ui.label(
                                                RichText::new(
                                                    entry["after_revision"].as_str().unwrap_or(""),
                                                )
                                                .small()
                                                .weak(),
                                            );
                                        });
                                        ui.label(
                                            RichText::new(format!(
                                                "Timestamp: {} ms since Unix epoch",
                                                entry["timestamp_ms"]
                                            ))
                                            .small()
                                            .weak(),
                                        );
                                        ui.separator();
                                    }
                                });
                            }
                            Err(e) => {
                                ui.label(e.to_string());
                            }
                        }
                        if let Some(path) = jenga_tools::journal::path(doc) {
                            ui.label(RichText::new(path.display().to_string()).small().weak());
                        }
                    }
                });
            self.history_dialog = open;
        }
        if self.text_dialog {
            let mut open = true;
            egui::Window::new(if self.tool == Tool::Label {
                "Place net label"
            } else {
                "Place text"
            })
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.text_edit_multiline(&mut self.text);
                if ui
                    .add_enabled(
                        !self.text.trim().is_empty(),
                        egui::Button::new("Place on canvas"),
                    )
                    .clicked()
                {
                    self.text_dialog = false;
                }
            });
            if !open {
                self.text_dialog = false;
                self.tool = Tool::Select;
            }
        }
        if self.chooser {
            self.symbol_chooser(ctx);
        }
        if self.library_dialog {
            let mut open = true;
            egui::Window::new("Library sources").open(&mut open).default_size([800.,480.]).show(ctx,|ui|{
            ui.label("Priority: user-added → project → KiCad tables → installed KiCad → Jenga bundle.");ui.label("Schematic-embedded symbols always determine existing designs. First matching library ID wins.");ui.separator();
            egui::ScrollArea::vertical().max_height(290.).show(ui,|ui|{if let Some(libs)=&self.libraries{for source in &libs.sources{ui.horizontal(|ui|{ui.label(RichText::new(&source.name).color(ACCENT));ui.label(&source.kind);});ui.label(RichText::new(source.path.display().to_string()).small().weak());ui.separator();}for warning in &libs.warnings{ui.label(RichText::new(warning).color(Color32::YELLOW));}}else{ui.spinner();}});
            ui.horizontal(|ui|{if ui.button("Add symbol library…").clicked()&& let Some(path)=rfd::FileDialog::new().add_filter("KiCad symbol library",&["kicad_sym"]).pick_file(){let name=path.file_stem().unwrap_or_default().to_string_lossy().into_owned();self.settings.libraries.insert(0,Source{name:format!("User / {name}"),path,kind:"symbols".into()});self.persist();self.reload_libraries();}
if ui.button("Add footprint folder…").clicked()&& let Some(path)=rfd::FileDialog::new().pick_folder(){self.settings.libraries.insert(0,Source{name:"User footprints".into(),path,kind:"footprints".into()});self.persist();self.reload_libraries();}
if ui.button("Rescan").clicked(){self.reload_libraries();}});
            let extras=self.settings.libraries.clone();for(index,source)in extras.iter().enumerate(){ui.horizontal(|ui|{ui.label(&source.name);if ui.small_button("Remove user source").clicked(){self.settings.libraries.remove(index);self.persist();self.reload_libraries();}});}
            ui.add_space(8.);ui.label("Symbols and 2D footprint previews are available. 3D models are not included.");
        });
            self.library_dialog = open;
        }
        if self.show_checks {
            let mut open = true;
            egui::Window::new("Electrical rules checker").open(&mut open).default_size([720.,450.]).show(ctx,|ui|{
                let errors=self.checks.iter().filter(|i|i.severity=="Error").count();
                ui.heading(format!("{} errors · {} warnings",errors,self.checks.len()-errors));
                ui.label("Single-sheet checks · Click an issue to locate the component.");
                ui.horizontal(|ui|{if ui.button("Run checks again").clicked(){self.inspect();}ui.checkbox(&mut self.erc_errors_only,"Errors only");});
                ui.separator();
                if self.checks.is_empty(){ui.label(RichText::new("No issues found by the implemented checks.").color(Color32::from_rgb(22,130,65)));}
                let issues:Vec<_>=self.checks.clone().into_iter().zip(self.check_details.clone()).collect();
                egui::ScrollArea::vertical().max_height(320.).show(ui,|ui|{for (issue,detail) in issues {
                    if self.erc_errors_only && issue.severity!="Error" {continue;}

                    let color=if issue.severity=="Error" {ACCENT} else {Color32::from_rgb(157,101,15)};
                    if ui.add_sized([ui.available_width(),36.],egui::Button::new(RichText::new(format!("{}  ·  {}",issue.severity,issue.message)).color(color))).on_hover_text(&detail.suggestion).clicked(){
                        let finding=detail.clone();
                        self.selected=issue.object;
                        if let (Some(i),Some(doc))=(issue.object,&self.doc) && let Some((a,b))=Scene::from_document(doc).object_bounds(i){self.view.center=((a.0+b.0)/2.,(a.1+b.1)/2.);self.view.scale=12.;}
                        if let Some(at)=finding.location {self.view.center=at;}
                        self.status=finding.suggestion.clone();self.active_issue=Some(finding);
                        self.show_checks=false;
                    }
                }});
                ui.separator();ui.label(RichText::new("Hierarchical connectivity and the full KiCad ERC rule matrix are not yet implemented.").small().weak());
            });
            self.show_checks &= open;
        }
        if self.about {
            egui::Window::new("About Jenga").open(&mut self.about).show(ctx,|ui|{ui.heading(format!("Jenga {}",env!("CARGO_PKG_VERSION")));ui.label("Open-source Rust schematic editor · GPL-3.0-or-later");ui.label("A: symbol · W: wire · J: junction · L: label · T: text\nQ: no-connect · M: move · R: rotate · Delete: remove\nHome: fit · Esc: cancel · Ctrl+S: save · Ctrl+Z: undo");ui.separator();ui.label("Install a newer Jenga Setup.exe to upgrade in place. Projects and settings are kept outside the installation folder. Automatic online updates are not configured.");ui.label("Modern KiCad files preserve unknown data. Legacy .sch import, PCB editing and full KiCad ERC are not implemented yet. KiCad NewStroke lettering and embedded symbol geometry are used.");ui.hyperlink_to("License","https://www.gnu.org/licenses/gpl-3.0.html");ui.hyperlink_to("KiCad library attribution","https://www.kicad.org/libraries/license/");});
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new("Jenga · message")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(error);
                    if ui.button("OK").clicked() {
                        self.error = None;
                    }
                });
        }
    }
}
