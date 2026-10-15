# Jenga 0.1

Run the versioned Windows Setup.exe to install Jenga. To use the portable download, extract its entire ZIP and run Jenga/Jenga.exe. Libraries are next to the executable; moving only the EXE loses the standalone library bundle.

Create a project from the home screen or open a .kicad_pro/.kicad_sch file. New projects contain a native KiCad project and schematic. Save and Save As write .kicad_sch. Existing files receive a .jenga-backup copy before replacement.

## Editing

Select objects by clicking; drag to move on the grid. Search/place symbols with A, choose a unit for multi-unit symbols, and click to place. W starts wiring: click points to route orthogonal segments, double-click to finish or Esc to stop. Use J for junctions at connected crossings. Use L for labels, T for text, Q for no-connect markers. R rotates the selected symbol, Delete removes it, Ctrl+Z undoes and Ctrl+Y redoes. M moves a selected object to the next click. Home fits the schematic, wheel zooms and middle/right drag pans.

Ctrl+O opens a schematic/project, Ctrl+N creates a project, Ctrl+S saves and Ctrl+Shift+S saves as. Unsaved changes prompt before opening another file or closing.

The Properties panel edits reference, value, footprint and datasheet for placed symbols. Preferences → Library sources shows all discovered paths and priority; user-added sources can be removed. Project/global KiCad tables (including custom entries) are scanned automatically. Existing embedded definitions take precedence over library updates.

## Current limits

- Modern .kicad_sch is supported. Older modern-version files retain their version/unknown fields, but universal backward compatibility has not been certified. Legacy .sch is explicitly rejected with instructions to convert in KiCad.
- Editing and checks are primarily single-sheet. Child sheets can be opened separately; hierarchical ERC is not implemented.
- Symbol geometry uses real embedded definitions. Text uses KiCad NewStroke lettering; advanced label outlines/pin decorations, alternate body styles and complex objects are not all rendered exactly like KiCad yet.
- Basic ERC is a subset of KiCad checks. Its findings are not a substitute for a full design review.
- SVG/PDF are A4 landscape exports. Text is exported as vector strokes with NewStroke Unicode glyphs.
- Footprints are included but PCB editing is not available. 3D models are excluded.
- Run a newer Setup.exe for an in-place upgrade. Online automatic updating is not configured.
