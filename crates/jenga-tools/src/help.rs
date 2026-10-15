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
            "Jenga is a Rust schematic editor with deterministic CLI, stdio MCP and JSON tools. Use tools/list or help topic=tools for argument schemas. Inspect current objects/revision, search native components, apply transactional edits with expected_revision, run ERC, capture the result and save explicitly. Desktop closed: mcp --workspace <existing folder>. Live desktop: mcp --connect auto. External actor labels record attribution; they are not credentials. Topic choices: overview, tools, editing, libraries, connections, limits, all."
        }
        "editing" | "libraries" | "connections" | "all" => GUIDE,
        _ => bail!(
            "Unknown help topic. Use overview, tools, editing, libraries, connections, limits or all"
        ),
    };
    Ok(json!({"version":env!("CARGO_PKG_VERSION"),"topic":topic,"content":content,"limits":LIMITS}))
}
