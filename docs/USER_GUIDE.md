# Jenga 0.4

Run the versioned Windows Setup.exe to install Jenga. To use the portable download, extract its entire ZIP and run Jenga/Jenga.exe. Libraries are next to the executable; moving only the EXE loses the standalone library bundle.

Create a project from the home screen or open a .kicad_pro/.kicad_sch file. New projects contain a native KiCad project and schematic. Save and Save As write .kicad_sch. Existing files receive a .jenga-backup copy before replacement.

## Editing

Select objects by clicking; drag to move on the grid. Press A to browse libraries or search names, descriptions and keywords (for example, `resistor` finds `Device:R`). Choose a unit, inspect the symbol, optionally select a footprint to preview and assign it, then press **Select**. The chooser closes: move to the canvas and click to place. R rotates the cursor preview. Esc or right-click cancels. Repeated copies are optional; otherwise placement finishes after one click. Click a component in the left Components panel to place another copy.

W starts wiring: click points to route orthogonal segments, double-click to finish or Esc to stop. Use J for junctions at connected crossings. Use L for labels, T for text, Q for no-connect markers. R rotates the selected symbol, Delete removes it, Ctrl+Z undoes and Ctrl+Y redoes. M moves a selected object to the next click. Home fits the schematic page, wheel zooms and middle/right drag pans.

Ctrl+O opens a schematic/project, Ctrl+N creates a project, Ctrl+S saves and Ctrl+Shift+S saves as. Unsaved changes prompt before opening another file or closing.

The Properties panel edits reference, value, footprint and datasheet for placed symbols. Preferences → Library sources shows all discovered paths and priority; user-added sources can be removed. Project/global KiCad tables (including custom entries) are scanned automatically. Existing embedded definitions take precedence over library updates.

The home screen previews your real recent schematics. Previews refresh when files change. For a selected imported KiCad component, use **Preview symbol / footprint** in Properties to inspect its embedded symbol and assigned footprint. A symbol without an assigned footprint displays that explicitly; select a footprint to see the 2D preview. No 3D models are used.

ERC shows error and warning counts, filters to errors, and displays suggested repairs on hover. Clicking an issue centers and marks the affected pin or object; Properties shows its suggested repair. Run checks again after editing. Export PDF or SVG for a vector drawing with the document's paper size and page frame; CSV exports one BOM row per component, including multi-unit parts.

The panels start narrower and can be resized. Grid dots adapt to zoom without changing the snapping increment. Disabling the grid leaves a clean bright canvas. Project cards are clickable across their whole surface and lift on hover. Dot/ring interface patterns and a Jenga logo loading animation stay outside the engineering drawing.

Use File → Page settings / title block or the Properties button to edit title, company, date, revision and nine comments. Imported metadata is retained in native KiCad fields, and the default worksheet appears in the canvas, PDF/SVG and offscreen screenshots.

Changes made through the human interface and external tools are journaled with actor labels. Save and external completion notifications appear in the interface. See [external tools](AUTOMATION.md) to use the separate CLI/MCP/JSON executable. Jenga contains no AI chat or built-in model service.

## Connect Assistant and Help

Home → Connect Assistant offers Codex, Claude Code, Cursor, Antigravity and generic stdio configuration. The recommended automatic mode works with the desktop open or closed through one connection. Advanced headless/live overrides are optional. Choose an existing workspace, review the entry and copy it, export a connection kit, or install an entry into that client's user-wide settings. Existing unrelated settings remain intact and receive a backup. Existing Jenga CLI entries are updated with a backup; entries owned by another command are refused. Refresh the client's MCP servers or start a new session afterward. There is no universal MCP registry shared by every assistant.

An optional `jenga-design` skill can be installed for Codex, Claude Code or Cursor. The exported kit contains the same SKILL.md for other skill-aware clients. This supplies workflow guidance; it does not install a model or sign you into an assistant account. The Windows Start menu also has a Connect Assistant shortcut.

Help → Features & help describes placement, wiring, checks, exports and shortcuts. Assistants can call `jenga_help` or run `jenga-cli help --json` / `help --topic editing` without opening a document.

The Components list uses smaller rows and scrolls independently. The BOM button and all placement tools stay fixed below it, including Move and Select. Export → BOM · Excel writes a real `.xlsx` workbook with a frozen header and filters; CSV is also available. Export → Native schematic saves a `.kicad_sch` copy without switching your active file. BOM exports count multi-unit components once and omit symbols excluded from the BOM.

The homepage Jenga dot lettering progressively writes, holds and dissolves. Background rings, dot waves and circuit signals animate outside the engineering drawing.

## Current limits

- Modern .kicad_sch is supported. Older modern-version files retain their version/unknown fields, but universal backward compatibility has not been certified. Legacy .sch is explicitly rejected with instructions to convert in KiCad.
- Editing and checks are primarily single-sheet. Child sheets can be opened separately; hierarchical ERC is not implemented.
- Symbol geometry uses real embedded definitions. Text uses KiCad NewStroke lettering; advanced label outlines/pin decorations, alternate body styles and complex objects are not all rendered exactly like KiCad yet.
- Basic ERC is a subset of KiCad checks. Its findings are not a substitute for a full design review.
- SVG/PDF use the schematic's paper size. Text is exported as vector strokes with NewStroke Unicode glyphs.
- Footprints are included but PCB editing is not available. 3D models are excluded.
- Run a newer Setup.exe for an in-place upgrade. Online automatic updating is not configured.

## 0.5 project lifecycle and engineering practices

- One automatic MCP/JSON connection works open or closed. No second server is required. `ui_state` reports the active document; explicit `open` chooses a different project. Failed edits are never replayed against another backend. Save before handoff. Unsaved edits block an unsafe handoff, and a timeout requires inspection before retrying.
- `create_project`: name and any absolute folder. `save`: any absolute schematic path. `save_project_as`: folder and name; copies the saved native project, libraries and assets to a new folder without overwriting the original. Use a dedicated project folder; ambiguous folders with multiple projects are refused.
- `delete_project`: archives the saved open project and returns recovery_folder/restore_manifest. Shared directories retain unrelated assets. Home cards have right-click Remove from Home and recoverable Delete actions.
- Home and clean open editors respond to external saves/renames within about one second. Unsaved edits trigger a conflict notice and block overwriting newer disk content. Save a separate copy or reopen explicitly.
- `connectivity` reports connected net names, exact pin memberships and wire counts. Verify intended pin pairs and isolated supplies after every block. It currently covers one sheet.
- `arrange_symbols` edit action: uuids array, at:[x,y], optional columns. It uses real geometry, fits inside the page and refuses wired symbols. Use it before wiring functional blocks; it is not a full circuit-layout optimizer.
- Prefer visible wires between nearby components and named labels for supplies/distant nets. Never substitute a giant sparse sheet and thousands of labelled stubs for readable circuit topology. Use `get_symbol` physical_pin_count/default_footprint, inspect all units and check the actual manufacturer package. No automatic DIP guessing.
- Existing inherited-symbol embeddings are repaired when saved/exported, including older Jenga files. Validate exports in KiCad where available. Physical performance is separate from ERC; sourcing, timing and bench validation must not be inferred from a clean schematic.
- Home has distributed dot waves, moving signal traces, animated headings and hover cards. Reduce motion is saved in settings; it applies to Home's animated patterns and headings.



### Copy, Save As and Delete (0.5.1)

Use Edit → Copy/Cut/Paste, or Ctrl+C/X/V. Shift-click adds/removes objects in a block selection. Ctrl+A selects all supported schematic objects; clicking without Shift returns to one-object selection. Paste lands at your last canvas cursor and is undoable. Copy text can also be pasted into KiCad's schematic editor; supported native KiCad clipboard text can be pasted back. Check references and net labels after transferring a circuit block.

Save As is visible in the toolbar: choose a schematic file or a complete project copy (including libraries/assets) in a new folder. Delete Project is visible beside it and archives recoverably. The confirmation offers export of a native schematic, PDF and Excel BOM to a separate destination first. Cancel or any export failure keeps the project. Home cards offer the same confirmation from their context menu.

Connect Assistant highlights Copy MCP Configuration and Download Jenga Skill, and includes View Skill. The CLI's `skill` command and MCP's `jenga_view_skill` / `jenga_export_skill` expose the same guide.
