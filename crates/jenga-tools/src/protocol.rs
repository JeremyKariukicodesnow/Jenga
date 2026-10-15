//! MCP stdio (negotiated 2025-11-25 / 2025-06-18) and newline JSON requests.
use anyhow::Result;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
pub const METHODS: &[&str] = &[
    "footprint_3d",
    "pcb_new",
    "pcb_open",
    "pcb_close",
    "pcb_save",
    "pcb_inspect",
    "pcb_edit",
    "pcb_connect",
    "pcb_preview",
    "pcb_commit",
    "pcb_verify",
    "pcb_trace",
    "pcb_sync",
    "pcb_assign_footprints",
    "pcb_autoplace",
    "pcb_autoroute",
    "build_pcb_from_schematic",
    "pcb_fill_zones",
    "pcb_export",
    "pcb_3d",
    "pcb_undo",
    "pcb_redo",
    "pcb_history",
    "footprint_save",
    "footprint_manage",
    "footprint_copy",
    "model_import",
    "model_inspect",
    "model_export",
    "pcb_create_project",
    "pcb_copy_project",
    "pcb_rename_project",
    "pcb_archive_project",
    "connect_block",
    "preview_edit",
    "commit_preview",
    "operation_status",
    "verify",
    "list_archived_projects",
    "restore_project",
    "help",
    "project_info",
    "save_symbol",
    "manage_symbol",
    "copy_symbol",
    "verify_connections",
    "trace_connection",
    "diagnose",
    "create_page",
    "delete_page",
    "rename_page",
    "reorder_pages",
    "export_pages",
    "view_skill",
    "export_skill",
    "copy_objects",
    "paste_objects",
    "open",
    "new",
    "create_project",
    "inspect",
    "ui_state",
    "search_symbols",
    "get_symbol",
    "get_footprint",
    "edit",
    "save",
    "save_project_as",
    "delete_project",
    "export",
    "screenshot",
    "erc",
    "connectivity",
    "history",
    "undo",
    "redo",
    "list_projects",
    "read_file",
    "import_library",
    "pointer",
    "set_view",
    "rename",
    "close",
    "create_schematic",
    "list_libraries",
    "search_footprints",
    "read_library",
    "write_library",
    "create_library",
    "preview_symbol",
    "preview_footprint",
    "ui_action",
];
fn description(method: &str) -> &str {
    if crate::pcb_tools::METHODS.contains(&method) {
        return crate::pcb_tools::description(method);
    }
    match method {
        "connect_block" => {
            "Route connections:[{from,to}] together atomically, preserving prior routes as obstacles. Exact pins require uuid/reference, number and optional unit. dry_run:true returns a guarded preview with proposed wires and indexed failures. No implicit save."
        }
        "preview_edit" => {
            "Preview transactional actions without applying them. Returns preview_id, proposed changes, verification and exact failures. Only a successful preview may be committed at its unchanged base revision."
        }
        "commit_preview" => {
            "Apply a successful preview_id atomically if the document identity, base revision and saved file are unchanged. Undoable; save explicitly."
        }
        "operation_status" => {
            "Read durable pending/completed/failed receipt by operation_id, including result or exact error. Reusing the same operation_id and request never repeats a mutation. Pending after process exit is indeterminate: inspect before issuing a new ID."
        }
        "verify" => {
            "One single-sheet verification: ERC, expected/forbidden pin connections, dangling wire endpoints and native KiCad roundtrip/missing-symbol warnings. Reports external_cli_checked:false; does not claim external KiCad verification."
        }
        "list_archived_projects" => {
            "List hidden Home projects and recoverably deleted project manifests, with restoration availability."
        }
        "restore_project" => {
            "Restore hidden Home project by path or recoverably deleted project by restore_manifest. Refuses occupied destinations and rolls back partial moves."
        }
        "project_info" => {
            "Read native project identity, folder, independent schematics and hierarchical child pages; optional path or current document."
        }
        "create_page" => {
            "Create a native KiCad schematic in the current project; hierarchical:true links a child sheet into the saved current page. Independent pages use hierarchical:false. Optional expected_revision. Saves both files with rollback on failure."
        }
        "export_pages" => {
            "Export all or selected project pages. PDF is one multi-page document, or separate:true makes one PDF per page in the path folder; PNG/SVG/native export a file per page into path folder. pages accepts exact page names or absolute files. Save first."
        }
        "manage_symbol" => {
            "Rename, recoverably backed-up delete, or export a custom project/personal symbol. action rename/delete/export; installed libraries are excluded. Mutations require expected_revision from read_library. Embedded placed instances are not rewritten."
        }
        "save_symbol" => {
            "Create/update a native KiCad symbol in library using structured pins/body/graphics or native_source. scope:'personal' shares across projects; scope:'project' stays local. Existing libraries require expected_revision from read_library. Footprint IDs are validated; native model links belong to footprints. Save does not refresh schematic embeddings automatically."
        }
        "copy_symbol" => {
            "Copy a resolved symbol from id into another library, preserving all units, geometry and footprint link. Default scope personal makes it reusable across projects. Preserve original license/attribution. Existing target library needs expected_revision."
        }
        "trace_connection" => {
            "Inspect actual pin connectivity including labels and T junctions. from/to are exact pin identifiers; returns net membership and involved wire UUIDs. No custom graph script required."
        }
        "diagnose" => {
            "Return revision/file/dirty state plus erc.issues and connectivity.nets in one stable object; compare live vs saved revision before mixing backends."
        }
        "verify_connections" => {
            "Check expected_connections and forbidden_connections arrays of {from,to} exact pin identifiers (uuid/reference, number, optional unit). Returns failures independently of ERC. The same arrays on edit reject failed intent atomically."
        }
        "view_skill" => {
            "Read the bundled versioned jenga-design SKILL.md, with wiring, clipboard, project and verification best practices. No open document required."
        }
        "export_skill" => {
            "Save/download the bundled SKILL.md to an absolute or workspace-relative path. Refuses existing destinations. No circuit is modified."
        }
        "copy_objects" => {
            "Copy UUID-selected supported flat schematic objects (omit uuids for all) as native KiCad clipboard text with embedded definitions. Does not touch the OS clipboard."
        }
        "paste_objects" => {
            "Paste native KiCad clipboard text or a full flat schematic into the current document. Optional at is the block anchor in mm. Fresh UUIDs/references, definitions retained, conflict aliases isolated. expected_revision recommended; undoable, explicit save needed. Rejects unsupported objects atomically."
        }
        "connectivity" => {
            "Inspect actual connected nets, names, pin numbers/UUIDs/coordinates and wire segment counts. Compare intended pin memberships after wiring; matching net labels are explicit electrical joins, bare crossings are not. Single-sheet scope."
        }
        "save_project_as" => {
            "Copy the saved native project, libraries and assets to folder/name without replacing the original. folder may be any absolute destination. Refuses existing destinations and folders containing multiple projects."
        }
        "delete_project" => {
            "Recoverably archive the saved open project. Returns recovery_folder and restore_manifest; removes its Home card. Refuses unsaved edits. Shared directories keep unrelated assets."
        }
        "help" => {
            "Discover Jenga features, CLI/MCP usage, argument schemas, modes and limits. Optional topic: overview, tools, editing, libraries, connections, limits, all. No open document needed."
        }
        "rename" => {
            "Rename the saved open schematic or its native project: name, kind:'schematic'/'project', optional project_path. move_folder:true explicitly moves a folder named after that project. Validates destinations, updates native instance metadata, preserves UUIDs and rolls back on failure. Save first. Does not overwrite existing destinations."
        }
        "close" => {
            "Close the document (return desktop to project home). Save first or explicitly discard_changes:true."
        }
        "create_schematic" => {
            "Create and save a new native schematic: path. Refuses existing files. Uses the existing project's directory and library tables."
        }
        "list_libraries" => {
            "Inspect all library sources, paths, kinds, symbol/footprint counts and discovery warnings."
        }
        "search_footprints" => {
            "Search all configured/bundled/installed 2D footprints by library ID: query, optional limit."
        }
        "read_library" => {
            "Read a native symbol library or footprint file: path. Returns complete source and revision for guarded authoring."
        }
        "write_library" => {
            "Create or edit project-native symbol library/footprint: name (library alias), source (full validated .kicad_sym/.kicad_mod text), actor, license/attribution. Updating an existing item requires expected_revision from read_library. Registers project tables and backs up prior files. No code execution. Existing schematic embeddings stay stable until explicit refresh_symbol or replace_symbol edits."
        }
        "create_library" => {
            "Create an empty registered project library: name, kind:'symbols'/'footprints'. Save the schematic first."
        }
        "preview_symbol" | "preview_footprint" => {
            "Render a colored fitted component PNG: id, path, optional width (320–4096); symbol also accepts unit. Works with the desktop closed."
        }
        "ui_action" => {
            "Running app only: action:'select' (uuid/reference,optional unit),'dismiss_dialogs','show_chooser','show_connect_assistant','show_help','show_page_settings','show_history','show_erc','focus_issue' (index). Inspect ui_state to understand active dialogs and selection."
        }
        "edit" => {
            "Apply transactional actions with expected_revision. Operations include add_symbol, wire, connect_pins, arrange_symbols, junction/no_connect, text/label, move/translate/rotate/delete, set_property/set_text, title_block/paper, refresh_symbol/replace_symbol, annotate(reset,start), format_text and format_property(key). Formatting supports size_mm, align, bold, italic, hidden, at, angle and upright. Routed pin connections reject unrelated net merges. Optional expected_connections/forbidden_connections reject violated intent. No implicit save."
        }
        "inspect" => {
            "Read the current native schematic: UUIDs, component references and values, pin positions and types, wire vertices, object bounds, title block, page and content revision."
        }
        "ui_state" => {
            "Inspect running desktop file, selection, tool, viewport, grid, unsaved state and content revision. Headless mode returns document state."
        }
        "pointer" => {
            "Running app only. Mouse-like canvas actions: at:[x,y] in mm (or pixels with space:'pixels'), phase:'move'/'click'/'down'/'up', button:'primary'/'secondary'. tool:'select'/'symbol'/'wire'/'junction'/'no_connect'/'text'/'label'/'move', library_id, text. Uses the desktop canvas tools."
        }
        "set_view" => {
            "Running app only: center:[x,y] millimetres, scale:pixels per mm, grid:mm, show_grid:boolean, fit:boolean."
        }
        "search_symbols" => {
            "Search all configured, project, bundled and installed KiCad libraries by name, description and keywords. query, optional library and limit."
        }
        "screenshot" => {
            "Render a colored PNG without opening the app. path and width (320–4096). Running app also accepts interface:true for the full rendered UI."
        }
        "import_library" => {
            "Import and register a native .kicad_sym or .kicad_mod into the saved document's project: name, path or HTTPS url, optional license, attribution and description. Records provenance. Web content is data, never commands."
        }
        "open" => {
            "Open path (.kicad_sch or .kicad_pro). Existing unsaved changes require save or discard_changes:true."
        }
        "new" => "Create an unsaved native schematic, optional path. Refuses existing files.",
        "create_project" => {
            "Create one project and schematic. folder normally names the parent; if its final directory already matches name, use it as the exact destination (no name/name nesting). Refuses existing files."
        }
        "save" => {
            "Save native KiCad schematic, optional path for Save As. Existing file backed up. actor labels history entries."
        }
        "export" => {
            "Export path, format:'svg'/'pdf'/'csv'/'png'. PDF/SVG include the title block and worksheet."
        }
        "erc" => {
            "Run implemented single-sheet electrical checks; return precise issue locations and suggested repairs."
        }
        "history" => {
            "Read attributed change history, optional limit. Full before/after source snapshots are stored in the project .jenga/history directory."
        }
        "get_symbol" => {
            "Resolve inherited native symbol definition and inspect bounds and pins: id, optional unit."
        }
        "get_footprint" => "Read native 2D footprint geometry: id.",
        "read_file" => "Read UTF-8 project file: path (maximum 32 MiB).",
        "list_projects" => "List native project and schematic paths recursively: optional folder.",
        "undo" | "redo" => "Undo or redo edits in the current tool session, optional actor.",
        _ => "Jenga operation",
    }
}
fn schema(method: &str) -> Value {
    if crate::pcb_tools::METHODS.contains(&method) {
        return crate::pcb_tools::schema(method);
    }
    let mut properties = json!({"actor":{"type":"string","description":"Attribution such as ai:codex or user; never an authentication claim."}});
    let keys: &[(&str, &str)] = match method {
        "operation_status" => &[("operation_id", "string")],
        "connect_block" => &[
            ("connections", "objects"),
            ("reserved_corridors", "objects"),
            ("dry_run", "boolean"),
            ("expected_revision", "string"),
            ("expected_connections", "objects"),
            ("forbidden_connections", "objects"),
        ],
        "preview_edit" => &[
            ("actions", "array"),
            ("expected_revision", "string"),
            ("expected_connections", "objects"),
            ("forbidden_connections", "objects"),
        ],
        "commit_preview" => &[("preview_id", "string"), ("expected_revision", "string")],
        "verify" => &[
            ("expected_connections", "objects"),
            ("forbidden_connections", "objects"),
        ],
        "inspect" => &[
            ("references", "strings"),
            ("uuids", "strings"),
            ("kinds", "strings"),
            ("include_pins", "boolean"),
            ("nets", "boolean"),
            ("net_names", "strings"),
            ("since_revision", "string"),
        ],
        "restore_project" => &[("path", "string"), ("restore_manifest", "string")],
        "copy_symbol" => &[
            ("id", "string"),
            ("library", "string"),
            ("scope", "string"),
            ("expected_revision", "string"),
            ("license", "string"),
            ("attribution", "string"),
        ],
        "manage_symbol" => &[
            ("library", "string"),
            ("scope", "string"),
            ("name", "string"),
            ("action", "string"),
            ("new_name", "string"),
            ("path", "string"),
            ("expected_revision", "string"),
        ],
        "save_symbol" => &[
            ("name", "string"),
            ("library", "string"),
            ("prefix", "string"),
            ("description", "string"),
            ("footprint", "string"),
            ("scope", "string"),
            ("pins", "objects"),
            ("width_mm", "number"),
            ("height_mm", "number"),
            ("graphics", "string"),
            ("shapes", "objects"),
            ("draw_body", "boolean"),
            ("value", "string"),
            ("replace_pins", "boolean"),
            ("native_source", "string"),
            ("expected_revision", "string"),
            ("license", "string"),
            ("attribution", "string"),
        ],
        "rename_page" => &[("path", "string"), ("name", "string")],
        "reorder_pages" => &[("pages", "strings")],
        "delete_page" => &[("path", "string")],
        "trace_connection" => &[("from", "object"), ("to", "object")],
        "diagnose" => &[],
        "verify_connections" => &[
            ("expected_connections", "objects"),
            ("forbidden_connections", "objects"),
        ],
        "project_info" => &[("path", "string")],
        "create_page" => &[
            ("name", "string"),
            ("hierarchical", "boolean"),
            ("at", "array"),
            ("page_number", "string"),
            ("expected_revision", "string"),
        ],
        "export_pages" => &[
            ("separate", "boolean"),
            ("path", "string"),
            ("format", "string"),
            ("pages", "strings"),
        ],
        "view_skill" => &[],
        "export_skill" => &[("path", "string")],
        "copy_objects" => &[("uuids", "strings")],
        "paste_objects" => &[
            ("text", "string"),
            ("at", "array"),
            ("expected_revision", "string"),
            ("actor", "string"),
        ],
        "help" => &[("topic", "string")],
        "rename" => &[
            ("name", "string"),
            ("kind", "string"),
            ("project_path", "string"),
            ("move_folder", "boolean"),
        ],
        "close" => &[("discard_changes", "boolean")],
        "create_schematic" => &[("path", "string")],
        "search_footprints" => &[("query", "string"), ("limit", "integer")],
        "read_library" => &[("path", "string")],
        "write_library" => &[
            ("scope", "string"),
            ("name", "string"),
            ("source", "string"),
            ("expected_revision", "string"),
            ("license", "string"),
            ("attribution", "string"),
            ("description", "string"),
        ],
        "create_library" => &[("name", "string"), ("kind", "string")],
        "preview_symbol" | "preview_footprint" => &[
            ("id", "string"),
            ("path", "string"),
            ("unit", "integer"),
            ("width", "integer"),
        ],
        "ui_action" => &[
            ("action", "string"),
            ("uuid", "string"),
            ("reference", "string"),
            ("unit", "integer"),
            ("index", "integer"),
        ],
        "open" => &[("path", "string"), ("discard_changes", "boolean")],
        "new" => &[("path", "string")],
        "create_project" => &[("name", "string"), ("folder", "string")],
        "edit" => &[
            ("dry_run", "boolean"),
            ("allow_named_net_merges", "boolean"),
            ("actions", "array"),
            ("expected_revision", "string"),
            ("expected_connections", "objects"),
            ("forbidden_connections", "objects"),
        ],
        "search_symbols" => &[
            ("query", "string"),
            ("library", "string"),
            ("limit", "integer"),
        ],
        "get_symbol" => &[("id", "string"), ("unit", "integer")],
        "get_footprint" => &[("id", "string")],
        "save" => &[("path", "string")],
        "save_project_as" => &[("folder", "string"), ("name", "string")],
        "export" | "screenshot" => &[
            ("path", "string"),
            ("format", "string"),
            ("width", "integer"),
            ("interface", "boolean"),
        ],
        "history" => &[("limit", "integer")],
        "list_projects" => &[("folder", "string")],
        "read_file" => &[("path", "string")],
        "import_library" => &[
            ("name", "string"),
            ("path", "string"),
            ("url", "string"),
            ("license", "string"),
            ("attribution", "string"),
            ("description", "string"),
        ],
        "pointer" => &[
            ("at", "array"),
            ("space", "string"),
            ("phase", "string"),
            ("button", "string"),
            ("tool", "string"),
            ("library_id", "string"),
            ("text", "string"),
            ("unit", "integer"),
            ("double", "boolean"),
            ("expected_revision", "string"),
        ],
        "set_view" => &[
            ("center", "array"),
            ("scale", "number"),
            ("grid", "number"),
            ("show_grid", "boolean"),
            ("fit", "boolean"),
        ],
        _ => &[],
    };
    if crate::operations::mutation(method) {
        properties["operation_id"] = json!({"type":"string","description":"Optional client-generated UUID for idempotent retry and durable status lookup."});
    }
    for (key, kind) in keys {
        properties[*key] = if *key == "actions" {
            json!({"type":"array","maxItems":10000,"items":{"type":"object","required":["op"],"properties":{"op":{"type":"string"}},"additionalProperties":true}})
        } else if *kind == "objects" {
            json!({"type":"array","maxItems":1000,"items":{"type":"object"}})
        } else if *kind == "strings" {
            json!({"type":"array","maxItems":10000,"items":{"type":"string"}})
        } else if *kind == "array" {
            json!({"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}})
        } else {
            json!({"type":kind})
        };
    }
    let required: Vec<&str> = match method {
        "connect_block" => vec!["connections"],
        "preview_edit" => vec!["actions"],
        "commit_preview" => vec!["preview_id"],
        "operation_status" => vec!["operation_id"],
        "manage_symbol" => vec!["library", "name", "action"],
        "rename_page" => vec!["name"],
        "reorder_pages" => vec!["pages"],
        "delete_page" => vec!["path"],
        "trace_connection" => vec!["from"],
        "save_symbol" => vec!["name", "library"],
        "copy_symbol" => vec!["id", "library"],
        "create_page" => vec!["name"],
        "export_pages" => vec!["path"],
        "export_skill" => vec!["path"],
        "paste_objects" => vec!["text"],
        "open" | "read_file" | "export" | "screenshot" | "read_library" | "create_schematic" => {
            vec!["path"]
        }
        "rename" | "create_library" => vec!["name"],
        "write_library" => vec!["name", "source"],
        "preview_symbol" | "preview_footprint" => vec!["id", "path"],
        "ui_action" => vec!["action"],
        "create_project" => vec!["name"],
        "save_project_as" => vec!["folder", "name"],
        "edit" => vec!["actions"],
        "get_symbol" | "get_footprint" => vec!["id"],
        "import_library" => vec!["name"],
        "pointer" => vec!["at"],
        _ => vec![],
    };
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
pub fn tools() -> Value {
    json!(METHODS.iter().map(|m|json!({"name":format!("jenga_{m}"),"description":description(m),"inputSchema":schema(m),"annotations":{"readOnlyHint":matches!(*m,"copy_objects"|"view_skill"|"help"|"inspect"|"ui_state"|"search_symbols"|"get_symbol"|"get_footprint"|"erc"|"history"|"read_file"|"list_projects"|"read_library"|"list_libraries"|"search_footprints"),"openWorldHint":*m=="import_library"}})).collect::<Vec<_>>())
}
pub fn mcp<R: BufRead, W: Write>(
    input: R,
    mut output: W,
    mut execute: impl FnMut(&str, &Value) -> Result<Value>,
) -> Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.len() > 1_048_576 {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                writeln!(
                    output,
                    "{}",
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})
                )?;
                output.flush()?;
                continue;
            }
        };
        let Some(id) = request.get("id") else {
            continue;
        };
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let p = request.get("params").cloned().unwrap_or(json!({}));
        let result = match method {
            "initialize" => Ok(
                json!({"protocolVersion":match p["protocolVersion"].as_str(){Some(v @ ("2024-11-05"|"2025-03-26"|"2025-06-18"|"2025-11-25"))=>v,_=>"2025-11-25"},"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"jenga-tools","version":env!("CARGO_PKG_VERSION")},"instructions":"Jenga exposes deterministic engineering tools. Inspect first; use UUIDs and expected_revision; edits do not save automatically. actor:'ai:<client>' attributes changes. Call jenga_help for capabilities and examples. No model service is bundled."}),
            ),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools":tools()})),
            "tools/call" => {
                let name = p["name"].as_str().unwrap_or("");
                if let Some(method) = name.strip_prefix("jenga_")
                    && METHODS.contains(&method)
                {
                    let mut args = p.get("arguments").cloned().unwrap_or(json!({}));
                    if let Some(map) = args.as_object_mut() {
                        map.entry("actor").or_insert(json!("ai:mcp"));
                    }
                    let call = execute(method, &args);
                    Ok(match call {
                        Ok(value) => {
                            json!({"content":[{"type":"text","text":serde_json::to_string(&value)?}],"structuredContent":{"result":value},"isError":false})
                        }
                        Err(e) => {
                            json!({"content":[{"type":"text","text":e.to_string()}],"isError":true})
                        }
                    })
                } else {
                    Err((-32602, "Unknown tool"))
                }
            }
            _ => Err((-32601, "Method not found")),
        };
        let response = match result {
            Ok(value) => json!({"jsonrpc":"2.0","id":id,"result":value}),
            Err((code, message)) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
            }
        };
        writeln!(output, "{response}")?;
        output.flush()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negotiates_supported_host_protocol_versions() {
        for v in ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"] {
            let request = format!(
                "{}\n",
                json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":v}})
            );
            let mut out = vec![];
            mcp(request.as_bytes(), &mut out, |_, _| Ok(json!({}))).unwrap();
            let response: Value = serde_json::from_slice(&out).unwrap();
            assert_eq!(response["result"]["protocolVersion"], v);
        }
    }
    #[test]
    fn mcp_stdout_is_protocol_only() {
        let input=b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"jenga_inspect\",\"arguments\":{}}}\n";
        let mut out = vec![];
        mcp(&input[..], &mut out, |_, _| Ok(json!({"test":true}))).unwrap();
        let lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[2]["result"]["isError"], false);
        assert_eq!(
            lines[1]["result"]["tools"].as_array().unwrap().len(),
            METHODS.len()
        );
    }
}
