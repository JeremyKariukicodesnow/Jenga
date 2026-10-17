# 0.8.1

- Darker, more physical PCB assembly presentation in the desktop 3D viewer, with restrained default brightness and ambient illumination for clearer solder-mask, copper and component contrast.
- 3D assembly views continue to export with the same brightness and ambient settings used for review.

# 0.8.0

- Stable assistant connection bridge survives installed-worker restarts and refreshes MCP tools after upgrade/downgrade. Interrupted requests are never replayed.
- Background preview/verification/export operations, durable receipts, cancellation, time budgets, bounded worker counts and desktop job inspection. Assistant-owned calculations use immutable desktop snapshots and survive desktop closure.
- Exact-part symbol lookup without whole-library indexing, bounded parsed-library cache, cheaper broad-phase copper checks and background snapshots without undo histories.
- Evidence-backed engineering review records with supply compatibility/current-budget findings; updated skill covers exact manufacturer evidence, full visible wires, readable labeling, ground/antenna reviews and honest hardware acceptance.
- Project/personal asset import from HTTPS or supplied files, retaining source/license information. Complete-project import, independent document import with private library aliases, and recoverable project relocation.
- Native PCB clipboard exchange with UUID/net/reference mapping and explicit named-net join preview, alongside existing schematic clipboard support.
- 3D assembly tab, separate window/fullscreen, correct linear GLB color conversion, brightness/ambient controls shared with exported camera views, and export-first format menus.
- Native netclass clearance inheritance, rejection of new rule findings, and connectivity-guarded duplicate copper cleanup. Native DRC remains required for complex KiCad geometry/rules.
- All offline libraries remain included; fast installer compression keeps release builds practical. No hardware certification is inferred from CAD verification.

# 0.7.1

- Explicit schematic/PCB targets for automation, precise schematic endpoint checks, and full-precision JSON coordinates.

- Preserve every pad UUID, position, number and net through native zone-fill conversion; stop on electrical conflicts.

- Guarded existing-board synchronization previews with footprint audit, changed pad nets, preserved routes and native DRC.

- Physical placement bounds exclude long value fields; finer packing respects courtyards and routing obstacles.

- Native backend receives project and bundled library tables; approximate zone clearance is distinguished from exact DRC.

- Dark PCB canvas, top/bottom layer controls, correct zone draw order/concave fill, icon ribbon, readable DRC and footprint repair findings, and reliable Project navigation.

- Fullscreen 3D assembly and accurate mechanical-hole model status; successful operations no longer masquerade as failed previews.

- Native component body creation for AI/humans using measured VRML primitives; compatible custom symbol/footprint/model linking.

- Save As and Open File Location in document ribbons; preset 3D views, PNG and multi-view PDF exports, native CAD exports, and named independent PCB pages with combined PDF export.

## 0.7.0 — 2026-10-14

- Native PCB-only and mixed projects, PCB canvas with pads/tracks/vias/outlines/zones/keepouts, native footprint editing and library cards.

- Guarded schematic synchronization, safe footprint replacement, batch routing, placement, preview/commit, compact inspection, history and durable background operation receipts.

- Native KiCad DRC and zone refill, Gerber/drill/BOM/placement and CAD exports; local STEP/VRML model resolution and orbitable 3D assembly viewer.

- Complete official symbols, footprints and 7,251 component model files bundled with the native backend; no separate KiCad installation required, including fallback from obsolete model-path settings.

- Sticky project Home navigation, scroll-to-top on entry, animated dot-font project name, visible project custom cards, collapsed main library groups with clear headings, dedicated archive restoration and project-folder access.

- Independent pages by default, explicit hierarchical sheet placement, and improved bundled schematic/PCB/footprint guidance.

- Maintenance installer launches assistant repair through a console host, records repair diagnostics, and skips the license for upgrade/repair/downgrade.

# Changelog

## 0.6.2 — 2026-10-14

- Atomic `connect_block` routing, guarded `preview_edit`/`commit_preview`, proposed wire/net changes and indexed failures. Optional reserved corridors and crossing penalties.

- Safe instance-specific symbol refresh/replacement with automatic number/name matching, explicit pin maps, wire preservation and short/split rejection.

- Durable operation IDs and pending/completed/failed receipts, idempotent mutation requests, responsive status queries and race-safe cancellation before execution.

- Compact component/pin/net inspection and changes since a known revision; combined verification returns ERC, intent, dangling endpoints and native KiCad compatibility findings.

- Conservative cleanup removes orphan dots, duplicate/zero-length wires and safe collinear joins while preserving electrical membership.

- Archived Home projects and recoverably deleted projects can be browsed and restored, with destination collision checks and rollback.

- Fixed-size clickable symbol cards, labelled expanded custom-library groups, side-by-side browsing/preview, and collapsed native source editing.

- Explicit independent/hierarchical page semantics and responsive project page cards.

- Upgrade/downgrade/repair use a maintenance path without repeated license agreement or installation choices. Fresh installations retain the license page.

- Updated working recipes and regression tests. Cross-sheet electrical analysis and hardware validation remain outside Jenga's implemented checks.

## 0.6.1 — 2026-10-13

- Editor page manager with switching, native renaming, title-block Save and close, recoverable deletion and sibling/descendant reordering.

- Project Home navigation, larger names, card utilities and toolbar regrouping; copy-project action remains in File.

- True write/erase heading animation, distinct navigation icons and contained utility labels.

- Omit native exported duplicates from schematic cards; no files deleted by discovery.

- Global/project symbol browser, source editing, promotion, native rename/export/delete and automatic library refresh.

- Simple recoverable Delete / Save and delete / Cancel project confirmation; no required export.

- Snapshot-based responsive live inspection, accurate connectivity revisions, named-net merge guards and trace/diagnose tools.

- Immediate subcommand help, exact project-folder handling, older MCP protocol negotiation and Hermes/OpenClaw setup snippets.

## 0.6.0 — 2026-10-13

- Expanded native symbol drawing (lines, polygons, rectangles, circles, arcs and labels), electrical pin types/styles, hidden pins and label-preserving edits.

- Antigravity skill installation, explicit skills-directory support for generic clients, PowerShell stdin/BOM request support, concurrent desktop discovery and expiry of queued requests.

- Native project workspace with schematic previews, independent/child page creation and selected multi-page PDF or per-page exports.

- Home card utility icons and schematic Save As/Delete/Project icons; darker animated headings and moving wireframe block motifs.

- Native symbol editor with pin types, body/graphics preview, existing-source editing, validated footprint links, project/personal scopes and copy_symbol promotion for reuse.

- Bounded routing search, unrelated-net merge rejection, expected/forbidden connection verification and transactional intent checks.

- Text/field font, alignment, position and upright formatting; reference annotation and upright rotation defaults.

- Update/Repair/Downgrade installation prompts and backed-up repair of existing assistant executable paths without changing workspace/mode arguments.

- Updated read/export skill and help explaining full explicit wiring, title blocks, functional descriptions, page workflows and library provenance.

- Native footprint model references are exposed/preserved. PCB/physical 3D rendering and full hierarchical ERC remain future work; no 3D model bundle added.

## 0.5.1 — 2026-10-13

- Route detours use the KiCad connection grid, straight routes omit redundant midpoint vertices, and exact imported pin coordinates are retained.

- ERC reports off-grid pins and wire endpoints, with UUIDs, exact focus positions and safe repair suggestions.

- Native schematic clipboard support: selected object, Shift-click block selection or all objects, Cut/Copy/Paste, cross-document transfer, embedded definitions, fresh UUIDs and references, multi-unit preservation and isolated definition conflicts. Unsupported object types are rejected atomically.

- Visible toolbar Save As and recoverable Delete Project actions. Both editor and Home deletion offer native/PDF/Excel BOM export first; cancellation or export failure keeps the project. Recovery manifest is written before moving files.

- Connect Assistant highlights Copy MCP Configuration and Download Skill, with selectable skill viewing. New MCP/JSON view_skill/export_skill and copy_objects/paste_objects tools; CLI skill command.

- Updated assistant skill and help covering grid, native clipboard, small wiring blocks, export-before-delete and verification limits.

## 0.5.0 — 2026-10-12

- Repair inherited symbol unit names when resolving libraries and saving/exporting existing schematics; preserve UUIDs and unknown fields.

- Correct PWR_FLAG handling so independent supply rails are not merged; expose real pin/net membership through connectivity.

- Exact pin snapping for wires/markers; conservative orthogonal detours with returned routes and failure instead of unsafe crossings. Compact geometry-aware arrangement for unwired symbols.

- Shared project events synchronize Home with CLI/assistant creates, renames and saves. Clean editors reload external changes; unsaved edits are preserved and conflicting saves are blocked.

- Recoverable project deletion, Home card context actions and complete saved-project copies to another user-selected location.

- One automatic MCP/JSON connection for desktop-open and desktop-closed operation; no failed mutation replay. Optional explicit headless/live overrides remain.

- Clearer assistant buttons, mouse-selectable configuration, migration of old Jenga entries, backed-up skill upgrades and detailed wiring/package/save best practices in machine help.

- More animated Home headings, distributed dot waves and travelling signals, with a saved Reduce motion setting. Large offscreen PNGs fit the pixel budget automatically.

- Bundled symbols/footprints and installer upgrade identity retained. No built-in AI chat, PCB editing or 3D models.

## 0.4.0 — 2026-10-12

- Independently scrolling compact Components list with a fixed placement-tools footer; Select and Move remain visible.

- Real Excel (.xlsx) BOM export with frozen header and filters, plus native schematic copy export in File/Export and external tools.

- Home Connect Assistant page: reviewed snippets, user-wide registration for Codex/Claude Code/Cursor/Antigravity, headless/live modes, connection-kit export and optional Jenga skill.

- Human Help page and machine-readable CLI/MCP help with capability schemas; 33 MCP operations.

- Animated dot-matrix Jenga write/hold/dissolve cycle, background dot waves, rings and travelling circuit signals.

- Assistant setup preserves unrelated settings and backs up JSON/TOML config; conflicting entries and existing skills are not overwritten.

- Windows Connect Assistant shortcut; installer upgrade identity preserved. No models, chat, PCB editing or 3D assets added.

## 0.3.0 — 2026-10-12

- Slimmer default panels, compact monospaced controls, solid dot-matrix lettering, bright adaptive grids and clickable project cards with animated hover lift.

- Subtle animated rings/dots on the project workspace and a breathing Jenga-logo loading state. Colored native schematic previews are retained.

- Editable native page/title metadata: title, company, date, revision and nine comments; shared worksheet zones/cells across the canvas and exports.

- ERC repair suggestions, UUIDs, precise locations and pulsing navigation markers; improved repeated/hidden power pin connectivity.

- Separate jenga-cli executable: headless CLI, MCP stdio, JSON-line control, authenticated loopback desktop connection, state inspection, pointer control and real/offscreen screenshots.

- Transactional document edits with revision guards, pin-aware wire stubs, placements, labels, text, no-connects, page metadata and attributed before/after history.

- Native project/schematic renaming with collision checks, UUID preservation and optional folder moves; guarded library authoring and fitted colored previews.

- Native symbol/footprint imports with project registration and provenance, recursive project inspection, batch recipes and dry runs.

- Installer/portable package include tools and client configuration examples. No built-in AI interface, model service, subscriptions or 3D assets.

Current limits remain documented: single-sheet ERC, default worksheet only, modern native formats, no PCB or obstacle-avoiding autorouting.

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
