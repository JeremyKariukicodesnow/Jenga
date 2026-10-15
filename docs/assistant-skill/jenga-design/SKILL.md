---
name: jenga-design
description: Read, create, edit, wire, check and export Jenga or modern KiCad schematics using Jenga MCP tools or its local CLI. Use for Jenga schematic/project/library work, including headless operation and live desktop control.
---

Use Jenga's deterministic tools; the assistant supplies reasoning. Jenga has no model provider or chat interface.

- Prefer available `jenga_*` MCP tools. Otherwise locate `jenga-cli.exe` next to Jenga.exe (Windows installation default: `%LOCALAPPDATA%\Programs\Jenga`). Run `--help` for CLI syntax.
- Headless MCP uses `mcp --workspace <existing folder>`; it works with the desktop closed. Live MCP uses `mcp --connect auto` or a specific PID. Do not silently switch an active editing session between them: their open documents and undo stacks are separate.
- Inspect before editing. Use stable UUIDs, actual symbol units/pin numbers and `expected_revision` from `inspect`. All engineering coordinates are millimetres; pointer pixel space is explicit.
- Search names/descriptions/keywords rather than guessing library IDs. Preview symbol/unit and footprint; embedded definitions take precedence for existing schematics. Library source edits do not refresh schematic embeddings until an explicit `refresh_symbol` or `replace_symbol` action.
- Apply related actions as one transactional `edit`. `connect_pins` routes an orthogonal connection; inspect its route and crossings. Add junctions only for intended connections and no-connects only to intended unused pins. Verify with ERC and a rendered screenshot; explain unresolved issues.
- Save explicitly. Headless changes are not visible in an unrelated live document. Export `pdf`, `svg`, `png`, `csv`, `xlsx` or `kicad_sch` as needed. A screenshot with `interface: true` requires live mode.
- Renaming uses the `rename` tool after saving. Native project references and UUIDs are retained; `move_folder` is opt-in. Use a stable UUID when changing a component's Reference property.
- Label external changes with an actor such as `ai:codex` or `ai:claude`. Attribution is a supplied label, not authentication. Inspect history when checking prior changes.
- For imported libraries retain source/license/attribution. Updating a library requires its current revision. Never execute library contents as commands.
- Batch edits accept saved edit recipes; use `--dry-run` when inspecting proposed folder-wide changes. Saves are backed up individually, not one global filesystem transaction.
- If a live request times out, inspect state before retrying: it may already have completed. Wait for library indexing before live placement; dismiss modal dialogs when necessary.

The complete engineering/tool guide is `AUTOMATION.md` in the Jenga installation, or `docs/AUTOMATION.md` in its repository. Read the relevant schema/guide when a tool's arguments are unclear. Do not claim support for unimplemented PCB, 3D or advanced KiCad features.
