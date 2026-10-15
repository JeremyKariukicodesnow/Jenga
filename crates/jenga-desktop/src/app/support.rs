use super::*;
impl App {
    pub(super) fn support_dialogs(&mut self, ctx: &egui::Context) {
        if self.connect_dialog {
            let mut open = true;
            egui::Window::new("Connect Assistant")
                .open(&mut open).default_width(650.).default_height(540.).vscroll(true)
                .show(ctx, |ui| {
                    ui.label("Use your own assistant. Jenga supplies engineering tools, with no model account or chat inside the app.");
                    ui.horizontal_wrapped(|ui| {
                        for client in ["Codex", "Claude Code", "Cursor", "Antigravity", "Other"] {
                            ui.selectable_value(&mut self.assistant_client, client.into(), client);
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.assistant_mode, "headless".into(), "Desktop closed / headless");
                        ui.selectable_value(&mut self.assistant_mode, "live".into(), "Live desktop");
                    });
                    ui.label(if self.assistant_mode == "live" {"Keep Jenga open. This entry connects to its newest responding window. Use a specific PID when several windows are open."} else {"Works with Jenga closed. The assistant opens files through its own tool session and saves explicitly."});
                    ui.horizontal(|ui| {
                        ui.label("Workspace");
                        ui.add(egui::TextEdit::singleline(&mut self.assistant_workspace).desired_width(360.));
                        if ui.button("Browse…").clicked() && let Some(p)=rfd::FileDialog::new().pick_folder() {
                            self.assistant_workspace=p.display().to_string();
                        }
                    });
                    let exe = jenga_tools::setup::executable();
                    let workspace=PathBuf::from(&self.assistant_workspace);
                    let home=jenga_tools::setup::home();
                    ui.label(RichText::new(format!("Tools: {}",exe.display())).small());
                    if let Ok(path)=jenga_tools::setup::config_path(&self.assistant_client,&home) {
                        ui.label(RichText::new(format!("User-wide configuration: {}",path.display())).small());
                    }
                    if let Ok(mut snippet)=jenga_tools::setup::snippet(&self.assistant_client,&self.assistant_mode,&workspace,&exe) {
                        if ui.button("Copy MCP configuration").clicked() {ctx.copy_text(snippet.clone());self.setup_status="Copied. Merge the entry into your client's MCP settings, then refresh its MCP servers.".into();}
                        egui::CollapsingHeader::new("Review configuration").default_open(true).show(ui,|ui| {
                            ui.add(egui::TextEdit::multiline(&mut snippet).code_editor().desired_width(f32::INFINITY).desired_rows(8).interactive(false));
                        });
                    }
                    ui.horizontal_wrapped(|ui| {
                        if self.assistant_client != "Other" && ui.button("Install MCP entry for this user").clicked() {
                            self.setup_status=match jenga_tools::setup::install(&self.assistant_client,&self.assistant_mode,&workspace,&exe,&home) {
                                Ok(p)=>format!("Installed {}. Existing settings retained; backup created. Refresh MCP or start a new assistant session.",p.display()),Err(e)=>e.to_string()
                            };
                        }
                        if ui.button("Export connection kit…").clicked() && let Some(folder)=rfd::FileDialog::new().pick_folder() {
                            self.setup_status=match self.export_connection_kit(&folder) {Ok(())=>format!("Connection kit saved in {}",folder.display()),Err(e)=>e.to_string()};
                        }
                    });
                    ui.label(RichText::new("Each assistant reads its own configuration. Installation adds one Jenga entry; conflicting entries are kept for you to review. The client may request approval before using tools.").small().weak());
                    ui.separator();
                    ui.label(RichText::new("Optional Jenga skill").strong());
                    ui.label("The skill teaches an assistant how to inspect, wire, verify, export and save using Jenga. It is guidance, not an AI model. The connection kit includes its SKILL.md.");
                    if ["Codex","Claude Code","Cursor"].contains(&self.assistant_client.as_str()) && ui.button("Install Jenga skill for this user").clicked() {
                        self.setup_status=match jenga_tools::setup::install_skill(&self.assistant_client,&home) {Ok(p)=>format!("Skill installed at {}. Reload skills or start a new session.",p.display()),Err(e)=>e.to_string()};
                    }
                    ui.separator();
                    egui::CollapsingHeader::new("CLI, JSON and assistant help").show(ui,|ui| {
                        ui.label("CLI runs terminal commands; MCP exposes structured tools; JSON sends explicit operations. All share the same Rust engineering core.");
                        let command=format!("& '{}' help --json",exe.display());
                        if ui.button("Copy assistant help command").clicked() {ctx.copy_text(command.clone());}
                        ui.monospace(command);
                        ui.label("MCP: call jenga_help with topic='overview', 'tools', 'editing', 'libraries', 'connections' or 'limits'. Tools/list supplies exact argument schemas.");
                        ui.label("Try: ‘Use Jenga tools to inspect my project, run ERC and capture a screenshot before editing.’");
                    });
                    let (name,url)=match self.assistant_client.as_str() {"Codex"=>("Codex MCP documentation","https://developers.openai.com/codex/mcp"),"Claude Code"=>("Claude Code MCP documentation","https://code.claude.com/docs/en/mcp"),"Cursor"=>("Cursor MCP documentation","https://cursor.com/docs/mcp"),_ => ("Antigravity MCP documentation","https://www.antigravity.google/docs/mcp")};
                    ui.hyperlink_to(name,url);
                    if !self.setup_status.is_empty() { ui.separator();ui.label(&self.setup_status); }
                });
            self.connect_dialog = open;
        }
        if self.help_dialog {
            let mut open = true;
            egui::Window::new("Jenga Help · features and workflow").open(&mut open).default_width(610.).vscroll(true).show(ctx,|ui| {
                ui.heading("Create · Place · Connect · Check · Export");
                for (title,body) in [
                    ("Projects and KiCad files","Create a project or open a modern KiCad schematic/project. Rename after saving. Native files retain UUIDs and unknown fields. The home cards preview your actual circuit."),
                    ("Components and tools","Only the component list scrolls. The fixed tool area below it keeps Select, Wire, Symbol, Junction, Label, Text, No connect and Move visible. Click a listed component for another copy."),
                    ("Search and placement","A opens symbol search across names, descriptions and keywords. Choose a unit and preview a footprint, press Select, then click the canvas. R rotates; Esc cancels. Repeated placement is optional."),
                    ("Wiring and navigation","W draws orthogonal segments; double-click finishes. J places a junction, Q a no-connect, L a label, T text. Wheel zooms, middle/right drag pans, Home fits. Grid display adapts while snapping keeps its chosen increment."),
                    ("Electrical checks","ERC lists issues, locations and possible repairs. Click one to focus and mark its affected pin/object. Recheck after editing; suggested repairs do not automatically change circuits."),
                    ("Title block and exports","Page settings edit native title/company/date/revision/comments. Export PDF/SVG, BOM as Excel or CSV, or a native .kicad_sch copy. Native export does not switch your active document."),
                    ("Assistant tools","Connect Assistant provides user-wide MCP configuration, headless/live modes, an optional skill and CLI/JSON help. No built-in model or chat. Call jenga_help to discover capabilities."),
                    ("History and updates","Ctrl+S saves, Ctrl+Shift+S saves as, Ctrl+Z/Y undo/redo. Change history records user/external actor labels. Install newer Setup releases in place. No online update host is configured."),
                ] {egui::CollapsingHeader::new(title).default_open(title=="Components and tools").show(ui,|ui|{ui.label(body);});}
                if ui.button("Connect Assistant…").clicked(){self.connect_dialog=true;}
                egui::CollapsingHeader::new("Current supported scope").show(ui,|ui|{ui.label(jenga_tools::help::LIMITS);});
            });
            self.help_dialog = open;
        }
    }
    fn export_connection_kit(&self, folder: &Path) -> anyhow::Result<()> {
        let exe = jenga_tools::setup::executable();
        let workspace = PathBuf::from(&self.assistant_workspace);
        for client in ["Codex", "Claude Code", "Cursor", "Antigravity", "Other"] {
            let ext = if client == "Codex" { "toml" } else { "json" };
            let path = folder.join(format!(
                "{}-{}.{}",
                client.replace(' ', "-"),
                self.assistant_mode,
                ext
            ));
            let content =
                jenga_tools::setup::snippet(client, &self.assistant_mode, &workspace, &exe)?;
            jenga_core::document::atomic_write(&path, content.as_bytes(), true)?;
        }
        jenga_core::document::atomic_write(
            &folder.join("jenga-design/SKILL.md"),
            jenga_tools::setup::SKILL.as_bytes(),
            true,
        )?;
        jenga_core::document::atomic_write(
            &folder.join("AUTOMATION.md"),
            jenga_tools::help::GUIDE.as_bytes(),
            true,
        )?;
        Ok(())
    }
}
