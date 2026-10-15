use anyhow::{Result, bail};
use serde_json::{Value, json};
pub const GUIDE: &str = include_str!("../../../docs/AUTOMATION.md");
pub const LIMITS: &str = "Modern native schematics and 2D footprints are supported. No PCB editor or 3D models. Legacy .sch conversion, full hierarchical ERC and custom worksheet rendering are not yet implemented. Explicit pin-aware routes are available; full autorouting is not. Compatibility is tested incrementally, not certified for every historic format. Updates use Setup; no online release host is configured.";
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
