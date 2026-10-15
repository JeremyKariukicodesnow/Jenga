# Jenga 0.7 PCB, footprint and 3D workflow

The complete Windows package includes official KiCad symbols, footprints and the installed 3D model library, plus the native PCB/3D backend. A separate KiCad installation is unnecessary. Standard `${KICAD10_3DMODEL_DIR}` (and older version) model links resolve against the bundled library; project models can remain relative to `${KIPRJMOD}`. Mesh inspection reports millimetres in engineering Z-up coordinates.

Projects may contain only PCBs, only schematics, or both. A board-to-schematic electrical link is optional. Open native `.kicad_pcb` files directly, or select a board card in the project dashboard. Native unknown fields are preserved. Save explicitly; previews do not edit the live design.

The PCB toolbar provides selection, moving, tracks, pad-to-pad routing, vias, outline polygons, copper zones, keepouts, text, rules, stackup, undo, verification and export. Select an object before Move, then click its destination. Double-click completes a polygon or track. Right/middle drag pans, and the wheel zooms. Safe moves and footprint replacement retain net assignments and reconnect existing tracks, or fail with exact pad/obstacle details. Inspect the result and run native DRC.

Main Home and project dashboards provide custom footprint libraries. Click a footprint card to edit pads, graphics or model transforms. Installed footprints are copied into a personal library before editing. Source editing is collapsed initially and explicitly applied, preserving native primitives outside the structured controls. Footprint definitions use `.kicad_mod` in `.pretty` libraries registered through `fp-lib-table`.

All 2D coordinates and dimensions are millimetres; rotations are degrees. STEP/STP and VRML are native footprint model formats. Standalone GLB/STL/OBJ meshes can be inspected and exported; these do not automatically become native KiCad footprint CAD models. Attach model paths and transforms to footprints, using `${KIPRJMOD}/models/...` for portable project assets. Missing models are reported. Models and rendering do not establish electrical safety.

The 3D viewer loads a native GLB assembly in the background, including board outline, holes, tracks, pads, zones and available component models. Drag to orbit, right-drag to pan, wheel to zoom, or use Top/Fit. The initial basic board preview is explicitly marked until native conversion completes. Native exports include STEP, GLB, STL, VRML, BREP and PLY.

## One-call engineering and AI tools

Read `help(topic:"tools")` for the installed schemas and `ui_state` for the intended active project. Use exact references/pad numbers, or `pad_uuid` for stacked pads. Do not infer pad numbering from physical order. Assign and validate footprints against datasheets before generating a board.

```json

{"method":"pcb_assign_footprints","params":{"footprints":{"R1":"Resistor_SMD:R_0805_2012Metric","C1":"Capacitor_SMD:C_0805_2012Metric"},"actor":"ai:codex"}}

{"method":"build_pcb_from_schematic","params":{"width_mm":80,"height_mm":60,"dry_run":true,"route":true,"actor":"ai:codex"}}

{"method":"pcb_commit","params":{"preview_id":"<returned preview_id>","actor":"ai:codex"}}

{"method":"pcb_verify","params":{"external":true}}

{"method":"pcb_save","params":{"path":"C:/Projects/MyBoard/MyBoard.kicad_pcb"}}

```

The build previews footprint validation, net assignment, connectivity-aware placement and bounded multilayer routing together. Missing footprint mappings stop synchronization before changing the board. Existing positions and unrelated board-only parts are preserved. Imported hierarchical designs use KiCad's native hierarchical netlist; independent drawings are not electrically merged. `pad_maps` maps each reference's schematic physical pin number to footprint pad number. Review placement and failed routes; the router is intended for modest boards, not high-speed or RF constraint closure.

`pcb_new` creates a board without a schematic. `pcb_create_project(name,folder)` creates a PCB-only native project. `pcb_sync` updates a linked board without automatic rerouting. `pcb_autoplace` refuses to move already routed footprints; explicit safe Move reconnects them. `pcb_autoroute` and `pcb_connect` support `dry_run`. Batches are atomic unless `allow_partial:true` is explicitly requested. New shorts prevent commit. A preview includes an image, added/changed/removed objects and indexed errors. `pcb_commit` checks both board and source schematic revisions.

```json

{"method":"pcb_connect","params":{"connections":[{"from":{"reference":"J1","number":"1"},"to":{"reference":"R1","number":"1"}}],"dry_run":true,"expected_revision":"<revision>"}}

{"method":"pcb_inspect","params":{"references":["J1","R1"],"include_pads":true}}

{"method":"pcb_verify","params":{"external":true,"expected_connections":[{"from":{"reference":"J1","number":"1"},"to":{"reference":"R1","number":"1"}}],"forbidden_connections":[]}}

```

Each accepted desktop PCB job returns an operation ID and `pending`. Query `operation_status` until `completed` or `failed`; a timeout is not permission to repeat an edit. Reusing the same ID/request returns the original receipt. Headless operations return a completed receipt synchronously. Never switch backends to retry an uncertain operation.

`pcb_verify` includes linked schematic ERC/dangling-wire findings (single-sheet scope), electrical intent, native geometric checks, widths, vias, unrouted nets, outline and model findings. `external:true` additionally runs full native KiCad DRC with project rules. `pcb_fill_zones` uses the same backend. Manufacturing export (`format:"manufacturing"`) creates Gerbers, drill files, placement CSV, BOM and verification report and blocks on failed native verification unless explicitly overridden. A clean DRC does not prove the circuit works.

Read only the requested objects using `pcb_inspect(references,uuids,kinds,include_pads,since_revision)`. `pcb_history`, `pcb_trace`, `pcb_undo` and `pcb_redo` support review. `footprint_save`, `footprint_copy`, `footprint_manage`, `model_import`, `model_inspect` and `model_export` provide native reusable resources. Library updates require a current resource revision. `pcb_copy_project`, `pcb_rename_project` and recoverable `pcb_archive_project` support saved-board maintenance.

Use `pcb_export` for native interchange and manufacturing/CAD outputs. Jenga reads modern KiCad PCB files directly and writes native files KiCad can open. KiCad backend operations are performed on temporary copies. Imported advanced geometry remains preserved; use native DRC for exact custom-pad/zone/courtyard rules. Full future-version support is not promised merely because an unfamiliar field survives a roundtrip.

## 0.7.1 editor and reliability

PCB editing uses a dark canvas by default. Red is front copper and blue is back copper; the active layer controls new tracks and zones, while visible-layer checkboxes independently control display. Fabrication and courtyard layers can be hidden without losing their native data. Filled copper zones are optional in the view and remain present in exports. The ribbon includes routing, vias, outline, zone filling, DRC, undo/redo and 3D. Export remains available in the inspector. Project returns to the saved project dashboard; save a new unsaved board first. The 3D window supports Full screen, Restore size, Fit and Top.

Build PCB first audits footprint assignments in a readable component list. Enter/search a `Library:Footprint` ID or create a custom footprint, apply assignments and recheck, then preview the build. PCB-only projects need no schematic; New PCB in a mixed project creates an independent board. A board built from a schematic may use `link:false` to remain electrically independent.

Explicit `target:"schematic"` / `target:"pcb"` selects the document for generic Save, Inspect, Verify and Export regardless of the active editor. `pcb_update_from_schematic` previews an existing linked board update with changed pad-net findings and native DRC. It preserves existing placement/routing and rejects source or revision conflicts. Commit only successful previews.

AI and human users can create simple native VRML component bodies. The footprint editor has measured body dimensions and box/cylinder creation. `model_create` additionally combines boxes, cylinders and spheres into a package with leads. Returned project-relative model paths can be attached to existing or custom footprints; symbols link through their Footprint library ID. Real package dimensions must still be checked. Nonplated mounting holes are intentionally model-less and no longer count as missing component models.


## Views, file actions and PCB pages

Both editors offer Save, Save As, Export and File Location. PCB Save As resolves project-relative component models against the original project so the new board retains its model links. The 3D ribbon offers seven standard camera views, current-view PNG/PDF, a seven-page PDF or PNG set, and STEP/GLB/STL/VRML/BREP/PLY assembly export. File Location opens the last exported assembly location or the board folder.

Add PCB page creates another named independent physical board in the project. Pages switches boards after saving; Export all pages PDF creates a single review document. Automation uses `pcb_create_page`, `pcb_export_pages`, and `pcb_export_3d_view`. PCB pages remain separate `.kicad_pcb` files for native interoperability.
