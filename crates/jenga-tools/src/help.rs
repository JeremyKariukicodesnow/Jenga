use anyhow::{Result, bail};
use serde_json::{Value, json};
pub const GUIDE: &str = include_str!("../../../docs/AUTOMATION.md");
pub const LIMITS: &str = "Native KiCad schematics, PCB-only and mixed projects, PCB canvas, footprint libraries, 3D assembly viewing and manufacturing exports are supported. Native KiCad backend provides advanced DRC, zone filling and STEP/VRML model conversion. Models are discovered locally or attached by the user. Routing is bounded and may report unfinished nets; placement needs review. Imported advanced fields are preserved, with restricted editing of custom primitives and generators. Cross-page schematic ERC remains single-sheet; hierarchical PCB synchronization uses native netlists. Legacy .sch conversion and custom worksheets are not supported. Setup supports upgrade, repair and downgrade; automatic online updates are not configured.";
pub fn get(topic: &str) -> Result<Value> {
    let content = match topic {
        "tools" => {
            return Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"tools":crate::protocol::tools(),"coordinates":"millimetres; pointer pixels require space:pixels","save":"edit does not implicitly save"}),
            );
        }
        "limits" => LIMITS,
        "overview" => {
            "Jenga is a Rust schematic and PCB editor with deterministic CLI, MCP and JSON tools. One connection: mcp --workspace <folder> --connect auto works with the app open or closed. Use tools/list or help topic=tools for schemas. Inspect UUIDs/units/pins/revision, place compact functional blocks, connect exact pin endpoints, call connectivity to verify intended nets, then ERC, screenshot and save explicitly. Workspace is a relative-path default; absolute create/save paths can be anywhere. save_project_as copies a saved project; delete_project archives recoverably. A clean ERC is not proof of a working circuit. External actor labels are attribution, not credentials. Topics: overview, tools, pcb, footprints, 3d, editing, libraries, connections, limits, all."
        }
        "pcb" | "footprints" | "3d" => include_str!("../../../docs/PCB_GUIDE.md"),
        "editing" | "libraries" | "connections" | "all" => GUIDE,
        _ => bail!(
            "Unknown help topic. Use overview, tools, editing, libraries, connections, limits or all"
        ),
    };
    Ok(json!({"version":env!("CARGO_PKG_VERSION"),"topic":topic,"content":content,"limits":LIMITS}))
}
