# Changelog

## 0.2.0 — 2026-10-12

- Bright cream workspace, red accents, original dot lettering, outlined toolbar icons and the supplied Jenga logo.
- Project cards preview real KiCad schematic geometry; previews are cached and refreshed when files change.
- Search all indexed symbols by name, description, inherited keywords and footprint. Browse grouped libraries, filter a library and display every matching result.
- Symbol/unit and 2D footprint previews, searchable footprint assignment and Select-to-place workflow. Selected imported components preview their embedded definition.
- Placed-components palette, optional repeated copies, cursor rotation, cancellation and automatic field positioning. Multi-unit placement shares references when a previous unit is available.
- Correct pin label alignment and ghost drawing without erasing underlying circuitry.
- ERC issue counts, severity filtering and navigation; additional power-driver, input-driver, net-name and detached-marker checks.
- Consistent page frames in the canvas, PDF and SVG, paper-size-aware export, missing-symbol guards and one BOM row per multi-unit component.
- Same Windows upgrade identity and per-user settings location. Symbols and footprints bundled; 3D models excluded.

Still an early release: legacy .sch import, full hierarchical ERC, PCB editing and automatic online updates are not available.

## 0.1.0 — 2026-10-12

Initial preview: Rust project manager and single-sheet schematic editor; modern KiCad file I/O preserving unknown fields; actual embedded symbol geometry; library discovery and inheritance; symbol placement, movement, rotation, wiring, labels, text, junctions, no-connect markers and undo/redo; basic ERC; SVG/PDF/BOM exports; Windows installer and portable packaging.

Symbols and footprints are bundled. 3D models are excluded. Legacy .sch editing, complete KiCad ERC, PCB editing, and automatic online updates are not yet available.
