---
name: jenga-design
description: Create, edit, wire, inspect and export Jenga or modern KiCad projects using Jenga MCP or CLI tools, with automatic desktop/headless operation.
---

Use Jenga's deterministic tools; the assistant supplies reasoning. There is no built-in chat or model provider.

- Prefer available jenga MCP tools. Otherwise locate jenga-cli.exe next to Jenga.exe and read --help. Use one automatic connection: `mcp --workspace <existing folder> --connect auto`. It uses the running desktop or works when closed. Save before switching. Failed edits are never replayed on another backend. `--headless` is an advanced isolation override; a PID selects a specific window.
- Call help with topic tools for current argument schemas. Inspect ui_state and explicitly open the intended project if another is active. Inspect before editing; use stable UUIDs, real units/pin numbers and expected_revision. Engineering coordinates are millimetres.
- Search names, descriptions and keywords instead of guessing component IDs. Embedded definitions take precedence for existing schematics. Inspect all units, including power units. get_symbol exposes physical_pin_count across all units and default_footprint. Never infer a package from unit 1's highest pin number. Verify manufacturer pinout and footprint preview.
- Work in small functional blocks. Use arrange_symbols on UUID-selected unwired symbols before routing. Prefer compact readable blocks and reasonable page sizes, with annotated signal flow; never scatter thousands of isolated labelled stubs as a substitute for readable wiring.
- connect_pins routes exact pin endpoints around other symbols and returns route_mm, or fails without changing the document. Explicit wire points support complex routes; do not round imported pin positions to a preferred grid. Crossings do not connect automatically. Add junctions only for intended branches. Use labels for supplies and distant connections; wire nearby related pins visibly.
- After each block, call connectivity. Confirm intended reference/pin pairs share a net and independent signals/supplies remain separate. Then run ERC and render a screenshot. Zero ERC findings alone do not establish correct wiring or a working physical circuit.
- Apply related actions in one transactional edit with expected_revision and actor ai:codex or ai:claude. Attribution labels are not authentication. Library updates also require their current revision; keep licensing/provenance and never execute library contents.
- Save explicitly. Project events update Home, and clean open documents reload external saves. Unsaved human edits are preserved on conflict. create_project accepts any absolute folder; create_schematic and save accept absolute paths. The workspace is a relative-path default, not a location restriction.
- save_project_as copies a saved project and its libraries/assets to folder/name without replacing the original. delete_project archives recoverably and returns a recovery location. Rename saved projects with rename; folder movement is opt-in. Inspect results and history.
- Native saves repair inherited symbol embedding names. Export PDF/SVG/PNG, Excel/CSV BOM or kicad_sch. When KiCad CLI is available, validate the native export with it. Do not certify interoperability merely because Jenga opens its own file. interface:true screenshots need a running desktop; offscreen PNGs fit the pixel budget.
- If a request times out, inspect before retrying: it may already have completed. Wait for library indexing before desktop edits and dismiss modals explicitly when needed.
- Batch recipes support dry runs and per-file backups; they are not a global filesystem transaction. Library source edits require explicit refresh_symbol/replace_symbol to update schematic embeddings.

Read AUTOMATION.md in the installation or docs/AUTOMATION.md in the repository for details. Current limits: single-sheet ERC, no PCB editor/3D assets, no legacy .sch conversion, no full multi-sheet automatic circuit synthesis. Explain unresolved engineering and sourcing limits honestly.
