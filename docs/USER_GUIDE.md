# Jenga 0.2

Run the versioned Windows Setup.exe to install Jenga. To use the portable download, extract its entire ZIP and run Jenga/Jenga.exe. Libraries are next to the executable; moving only the EXE loses the standalone library bundle.

Create a project from the home screen or open a .kicad_pro/.kicad_sch file. New projects contain a native KiCad project and schematic. Save and Save As write .kicad_sch. Existing files receive a .jenga-backup copy before replacement.

## Editing

Select objects by clicking; drag to move on the grid. Press A to browse libraries or search names, descriptions and keywords (for example, `resistor` finds `Device:R`). Choose a unit, inspect the symbol, optionally select a footprint to preview and assign it, then press **Select**. The chooser closes: move to the canvas and click to place. R rotates the cursor preview. Esc or right-click cancels. Repeated copies are optional; otherwise placement finishes after one click. Click a component in the left Components panel to place another copy.

W starts wiring: click points to route orthogonal segments, double-click to finish or Esc to stop. Use J for junctions at connected crossings. Use L for labels, T for text, Q for no-connect markers. R rotates the selected symbol, Delete removes it, Ctrl+Z undoes and Ctrl+Y redoes. M moves a selected object to the next click. Home fits the schematic page, wheel zooms and middle/right drag pans.

Ctrl+O opens a schematic/project, Ctrl+N creates a project, Ctrl+S saves and Ctrl+Shift+S saves as. Unsaved changes prompt before opening another file or closing.

The Properties panel edits reference, value, footprint and datasheet for placed symbols. Preferences → Library sources shows all discovered paths and priority; user-added sources can be removed. Project/global KiCad tables (including custom entries) are scanned automatically. Existing embedded definitions take precedence over library updates.

The home screen previews your real recent schematics. Previews refresh when files change. For a selected imported KiCad component, use **Preview symbol / footprint** in Properties to inspect its embedded symbol and assigned footprint. A symbol without an assigned footprint displays that explicitly; select a footprint to see the 2D preview. No 3D models are used.

ERC shows error and warning counts, allows filtering to errors, and locates a component when an issue is clicked. Run checks again after editing. Export PDF or SVG for a vector drawing with the document's paper size and page frame; CSV exports one BOM row per component, including multi-unit parts.

## Current limits

- Modern .kicad_sch is supported. Older modern-version files retain their version/unknown fields, but universal backward compatibility has not been certified. Legacy .sch is explicitly rejected with instructions to convert in KiCad.
- Editing and checks are primarily single-sheet. Child sheets can be opened separately; hierarchical ERC is not implemented.
- Symbol geometry uses real embedded definitions. Text uses KiCad NewStroke lettering; advanced label outlines/pin decorations, alternate body styles and complex objects are not all rendered exactly like KiCad yet.
- Basic ERC is a subset of KiCad checks. Its findings are not a substitute for a full design review.
- SVG/PDF use the schematic's paper size. Text is exported as vector strokes with NewStroke Unicode glyphs.
- Footprints are included but PCB editing is not available. 3D models are excluded.
- Run a newer Setup.exe for an in-place upgrade. Online automatic updating is not configured.
