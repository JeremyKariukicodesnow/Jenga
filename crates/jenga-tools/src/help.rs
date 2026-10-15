use anyhow::{Result, bail};
use serde_json::{Value, json};
pub const GUIDE: &str = include_str!("../../../docs/AUTOMATION.md");
pub const LIMITS: &str = "Native KiCad projects, multiple independent schematics, hierarchical page creation/navigation, selected multi-page export and native symbol libraries are supported. Connectivity and ERC remain single-sheet; verify cross-page electrical intent in KiCad. No PCB editor, physical schematic 3D assembly renderer, legacy .sch conversion or custom worksheet renderer. Native footprint 3D model links are exposed/preserved; models are not bundled. Routing and library editing are bounded deterministic operations, not circuit synthesis or hardware certification. Unknown newer KiCad fields are preserved; future visual/format coverage is not certified. Setup supports update/repair/downgrade; online automatic updates are not configured.";
pub fn get(topic: &str) -> Result<Value> {
    let content = match topic {
        "tools" => {
            return Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"tools":crate::protocol::tools(),"coordinates":"millimetres; pointer pixels require space:pixels","save":"edit does not implicitly save"}),
            );
        }
        "limits" => LIMITS,
        "overview" => {
            "Jenga is a Rust schematic editor with deterministic CLI, MCP and JSON tools. One connection: mcp --workspace <folder> --connect auto works with the app open or closed. Use tools/list or help topic=tools for schemas. Inspect UUIDs/units/pins/revision, place compact functional blocks, connect exact pin endpoints, call connectivity to verify intended nets, then ERC, screenshot and save explicitly. Workspace is a relative-path default; absolute create/save paths can be anywhere. save_project_as copies a saved project; delete_project archives recoverably. A clean ERC is not proof of a working circuit. External actor labels are attribution, not credentials. Topics: overview, tools, editing, libraries, connections, limits, all."
        }
        "editing" | "libraries" | "connections" | "all" => GUIDE,
        _ => bail!(
            "Unknown help topic. Use overview, tools, editing, libraries, connections, limits or all"
        ),
    };
    Ok(json!({"version":env!("CARGO_PKG_VERSION"),"topic":topic,"content":content,"limits":LIMITS}))
}
