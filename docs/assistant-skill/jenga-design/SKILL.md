---
name: jenga-design
description: Create, edit, fully wire, inspect and export Jenga or modern KiCad projects using Jenga MCP or CLI tools, with desktop/headless operation and reusable native symbol and footprint libraries, PCB routing and 3D assemblies.
---

Use Jenga's engineering tools. The assistant supplies reasoning; Jenga has no built-in chat or model provider. Read `help` topic `tools` for the installed version's schemas before relying on an operation. PCB tools below require Jenga 0.7 or later; use available operations on older versions and explain any limit.

## Connect and select the intended project

- Prefer Jenga MCP. Otherwise use the installed `jenga-cli.exe`. One `mcp --workspace <existing folder> --connect auto` connection works with the app open or closed. Register the stable installed path, never a temporary or versioned download folder.
- Read `ui_state`, then explicitly `open` the intended project. Preserve unsaved human edits. Use `inspect`, UUIDs, exact physical pin numbers, units and `expected_revision`. Coordinates are millimetres.
- Save before switching documents or backends. Never replay a failed mutation on another backend. After a timeout, inspect before retrying because the operation may have completed. Wait until indexing finishes before live edits.
- `view_skill` returns these instructions; `export_skill(path)` downloads them without opening a circuit. CLI: `jenga-cli skill`, `jenga-cli skill --output PATH`, `jenga-cli help --json`. JSON methods are the MCP names without `jenga_`.

## Plan a readable, complete schematic

- Work in functional blocks: supply/protection, inputs, controller, drivers and loads. Use compact placement, real symbol bounds, signal flow and routing corridors. Choose the smallest page that remains legible. Use `arrange_symbols` before wiring; it refuses wired symbols.
- Search names, descriptions and keywords; do not guess IDs. Embedded definitions take precedence. Inspect every unit, including power units. `get_symbol` exposes physical pin count and default footprint; `get_footprint` exposes pads and native model links. Confirm the actual manufacturer pinout and package.
- Assign unique references and meaningful values. Keep all units of one physical part under its shared reference. `annotate` preserves assigned references by default; ambiguous unannotated multi-unit parts require explicit grouping. Do not renumber imported circuits casually.

## Full explicit wiring is the default

Unless the user requests a label-based drawing or a cross-page connection requires it, draw continuous visible wires from component to component. Do not leave a circuit as scattered labelled stubs or substitute matching labels for the visible connections the user expects.

- Show the entire control path: controller pin -> driver -> relay/contactor/load, including returns. Show supply distribution, ground/neutral/earth as appropriate, coil supplies, clamps, sensor dividers, pullups, decoupling and connector destinations. Intentional isolation barriers remain electrically isolated; full wiring does not mean joining all grounds.
- Prefer `connect_pins` with exact `{uuid or reference, number, unit}` endpoints. It returns the route or fails atomically. It rejects unrelated net merges. Inspect the returned path. If routing fails, improve placement or choose explicit orthogonal points through a clear corridor; do not force a diagonal wire or bypass an isolation barrier.
- For explicit `wire(points)` edits, include expected and forbidden connection intent in the transactional edit. Inspect the actual nets afterward; geometry alone cannot prove correctness. Preserve imported endpoint coordinates. Use 1.27mm electrical positions for new designs, and never independently snap existing connected wires.
- Crossings are not connections. Put junction dots at intended branches. Avoid unrelated pins, labels, existing endpoints and no-connect markers. Never put a junction at an unrelated crossing merely to remove an ERC warning.
- Net labels may name a fully visible wire for readability. Across pages, use deliberate global/hierarchical connections with matching names and clear destination/page annotations. Do not assume all local labels join across sheets. Explain cross-page links and verify them in KiCad; Jenga's connectivity/ERC scope is currently one sheet.
- Place no-connect markers only on deliberately unused pins. Do not mark a pin no-connect to conceal a missing connection. Explain off-board devices and terminals clearly.

## Professional annotation and title blocks

- Fill `title_block` with the circuit's actual title, revision, date and project/company identity. Use the current date when creating a revision; retain imported dates unless revising intentionally. Comments should state function, supply/ratings, interfaces and unresolved component-selection requirements. Do not claim hardware testing or compliance from ERC.
- Identify each functional area and destination: room names, channels, inputs/outputs and connector pin functions. Label relay versus contactor, controller versus driver, supply versus regulator. Values should state part identity/rating; use separate text for long explanations instead of crowding pins.
- Add a short "How it works" description in clear empty space. Explain the user's actions, automatic behaviour, signal flow, startup state and external interfaces. Include a reference-prefix legend when useful.
- Use native `text` with `size_mm` and `align`, or `format_text` / `format_property(key)`. These support size, alignment, bold/italic, position, angle, hidden and upright. Use 1.27–1.8mm body/field text and 2–3mm section headings for new drawings; size the page for readable density. Place explanatory text in reserved empty areas, not across wiring or pins. Make headings larger than component fields and inspect for overlaps. Keep reference/value fields upright. Do not alter symbol pin-label sizes to hide an incorrect pinout.

## Projects, pages and reusable libraries

- `create_project` accepts any absolute folder. `project_info` lists project identity and pages. `create_page(name,hierarchical:false)` creates an independent drawing by default. Use `hierarchical:true` only when the user intends an electrically linked child: it inserts a real sheet rectangle into the saved parent. Never create linked sheets merely to paginate independent drawings. Never let sheet rectangles overlap an existing circuit. Save first. Open the selected page explicitly before editing.
- `export_pages` exports all or selected pages: one multi-page PDF or individual PNG/SVG/native files in a folder. `separate:true` also exports one PDF per page into the path folder. New projects start with one schematic/page; independent schematics and child pages are added explicitly. Selection accepts exact page names or absolute file paths. Preserve project page order. Check exported page count and title blocks.
- `save_symbol` authors a native `.kicad_sym` definition using structured pins/body/graphics or `native_source`. `scope:'project'` keeps it local; `scope:'personal'` makes it searchable in other projects. Personal libraries live outside the installation. Existing libraries require `expected_revision` from `read_library`.
- `copy_symbol(id,library,scope)` copies a resolved definition, keeping geometry, units and footprint links. Use it to promote a project symbol to a reusable personal library. Preserve the original license/attribution; do not relabel borrowed library data as CC0.
- A symbol's Footprint property uses a native `Library:Footprint` ID. 3D models and their transforms belong to the footprint; preserve those links. A schematic's 2D positions do not define physical PCB placement. Use PCB and 3D operations below for physical layout; inspect missing model findings.
- Library changes do not silently replace embedded schematic definitions. Use `refresh_symbol` or `replace_symbol` explicitly, and verify physical pins, wiring and footprint mapping afterward.
- `save_project_as` copies a saved project and its libraries/assets. `rename` keeps native project references consistent. Before `delete_project`, export useful native/PDF/BOM artifacts outside the project folder; deletion is recoverable and returns a recovery manifest. Do not modify unrelated projects.

## Verify and deliver

- After each functional block, inspect `connectivity`. Check intended pin pairs, distinct signals/supplies and isolation barriers. `verify_connections` checks `expected_connections` and `forbidden_connections`; the same fields on `edit` reject failed intent atomically.
- Run ERC, inspect every finding's location and fix the underlying issue. Zero ERC does not establish intended wiring, a working circuit, suitability for mains installation or local availability.
- Render a coloured screenshot/PDF and inspect labels, junctions, field orientation, wire crossings, worksheet, page fit and margins. Fix illegible or oversized layouts before presenting them as complete.
- Save explicitly with `actor:'ai:<client>'` for attribution. Clean open editors reload external saves; conflicts preserve human edits. When KiCad CLI is available, independently parse the native export and compare intended connectivity. State exactly what was verified and what remains untested.
- Copy/paste uses native clipboard text via `copy_objects` / `paste_objects`; paste creates new UUIDs/references and is undoable. Named labels can join existing nets: verify afterward. This does not manipulate the OS clipboard. Batch recipes have dry runs and per-file backups, not a global filesystem transaction.

For maintained schemas and examples, use `help` topic `tools` / `editing` / `libraries` or read `AUTOMATION.md` from the installed package. Current limits include single-sheet electrical analysis, no legacy `.sch` conversion, no PCB editor and no schematic-to-physical-3D assembly renderer. Unknown fields in newer modern KiCad files are preserved; full future-format compatibility is not certified.

## Reliable Windows and client setup

- In Windows PowerShell use a UTF-8 request file or pipe `ConvertTo-Json` output to `jenga-cli run --request -`. Inline JSON arguments can lose quotation marks at the shell boundary; do not repair malformed JSON by guessing missing values.
- Automatic mode checks the UI promptly. Native file dialogs can block live editing. Dismiss them before tool work; after a failed mutation inspect revision/history before retrying. Never repeat a timed-out write on a second backend. Use headless inspection with an explicit saved file when the desktop cannot respond, keeping unsaved human edits separate.
- `setup --client Antigravity --install --skill` installs its MCP entry and skill. Other clients can use the generic stdio snippet and `setup --client Other --skill --skill-dir ABSOLUTE_SKILLS_FOLDER`; choose the directory documented by that client. Hermes, OpenClaw and other hosts require MCP stdio support or can invoke CLI/JSON. Do not assume they all use the same configuration location.
- Structured symbol `shapes` support rectangle, circle, arc, line/polyline/polygon and text labels. Pins support names/numbers, electrical types, graphic styles, units, `length_mm` and `hidden`. Preserve native pin-label effects when adapting imported symbols. Native-source graphics can be preserved or edited directly; do not confuse symbol labels with schematic net labels.

## v0.6.1 diagnostics and client entry points

- Every CLI subcommand accepts `--help` and exits immediately; `mcp --help` does not start a server. Stdio MCP is newline JSON-RPC on stdin/stdout, not HTTP. The authenticated desktop loopback socket is an internal JSON-line channel; do not probe it with Invoke-WebRequest or kill a desktop with unsaved edits.
- `create_project(name,folder)` normally uses folder as the parent; if its basename already matches name it uses that exact destination. Check the returned file path, and never rearrange generated files using shell moves.
- `connectivity`, `diagnose`, `trace_connection` and `project_info` have CLI entry points. `diagnose` returns `erc.issues` and `connectivity.nets`; legacy `erc` returns an array of findings. Net names are in `names`, not `name`/`labels`. Revisions are consistent with `inspect`.
- Use `trace_connection(from,to)` for actual connectivity, including T contacts and named-net connections. Do not build endpoint-only graphs or compare points within arbitrary tolerances; they can miss contacts or invent shorts. `verify_connections` expresses the intended pairs/isolation.
- Edits reject new merges of differently named nets by default. `allow_named_net_merges:true` is an explicit intentional-alias override; never use it to suppress a short. Remove the incorrect wire or misplaced label instead. Place symbols on the connection grid and use returned pin coordinates.
- Live read-only queries use the desktop's published snapshot; `ui_state.snapshot_age_ms` / `diagnose.snapshot_age_ms` report freshness. Inspection stays responsive during native dialogs. Mutations run on the UI thread and are not replayed. Close dialogs/restore the window for editing; compare revisions and save before inspecting from a different headless process.
- `manage_symbol` provides rename/delete/export of custom project/personal definitions with revision guards; placed schematic embeddings remain intact. `delete_page` archives a saved non-root page and removes its native parent reference. Library changes should refresh UI lists automatically.
- Hermes has a `mcp_servers` YAML mapping; OpenClaw has `mcp.servers` with stdio transport. Their setup snippets are host-specific. Use the official client configuration instructions and reload the host; generic stdio support is not proof that every host version was tested.

- `rename_page` updates a native page title and parent Sheetname without renaming files/UUIDs. `reorder_pages` updates sibling sheet order and native page numbers; supply each project page path once, keep parents before children and preserve independent root order. Save first. Root deletion uses delete_project. The editor’s Pages manager exposes these operations plus title-block editing and export.

## v0.6.2 block workflows

- Prefer `connect_block(connections:[{from,to}],expected_revision)` for a functional block. `dry_run:true` returns proposed wires/nets, indexed failures, verification and a `preview_id`. `preview_edit` previews any actions batch. `commit_preview(preview_id)` applies only successful unchanged previews atomically; save explicitly afterward.
- Generate a UUID `operation_id` before mutations. After timeout, poll `operation_status` for pending/completed/failed and the retained result. Reusing the same ID/request never repeats edits. Pending after a process crash is indeterminate: inspect/recover before using a new ID.
- `replace_symbol`/`refresh_symbol` preserve wiring with one-to-one old/new physical pin matching; use `pin_map:{"old":"new"}` for ambiguity. All old pins need mappings. Short/split/unsafe routing rejects the batch. The selected embedding is isolated from other instances. Preview and verify after replacement.
- `inspect` selectors references/uuids/kinds/include_pins/nets/net_names and since_revision keep results compact. `verify` combines ERC, expected/forbidden pairs, dangling endpoints and native compatibility checks in one single-sheet call. Cross-sheet intent still requires KiCad.
- `{op:"cleanup"}` conservatively removes unattached dots, duplicate/zero-length wires and safe collinear joins; preview the removal list. `list_archived_projects`/`restore_project` expose Home archive/recovery.


## PCB and footprint workflow (0.7+)

- A project may contain a PCB alone or schematics and optional linked PCBs. Use `pcb_create_project` / `pcb_new` for board-only work, `pcb_open` for native KiCad boards, and `pcb_inspect` for exact pads, nets, layers and revision. Do not treat schematic coordinates as PCB placement.
- Validate package, physical pad numbering, pin-to-pad mapping, board dimensions, connector positions, mounting holes, layer stack, current/clearance rules and isolation barriers. Use `pcb_assign_footprints(footprints,pad_maps)` before synchronizing. Ambiguity must fail; never silently invent a mapping.
- Prefer one `build_pcb_from_schematic(dry_run:true)` call for validated footprint sync, placement and routing. Review its image, changes, failures and verification, then `pcb_commit(preview_id)` at unchanged revisions. Use `pcb_sync`, `pcb_autoplace`, `pcb_autoroute` or `pcb_connect` for focused work. Partial routing is unfinished work: report exact failed pad pairs, improve placement/corridors and retry a fresh preview. Do not force unsafe tracks or claim complete routing from the number of segments.
- Batch `pcb_edit`/`pcb_preview` actions. Respect pad net assignments, unconnected pads, keepouts, board cutouts and reserved corridors. Use safe `replace_footprint` with explicit `pad_map`; preserve net assignments and existing routing. Check all units of one physical schematic component and use native hierarchical netlists for linked pages; never merge independent drawings by matching local names.
- Use `pcb_verify(external:true,expected_connections,forbidden_connections)` before manufacturing. Review unrouted nets, shorts, clearances, via/drill dimensions, outline, silkscreen/courtyard findings and model availability. Fill zones explicitly before re-verifying. DRC and a plausible 3D rendering are not hardware verification.
- Use `footprint_save` for project/personal `.pretty` libraries, guarded by `read_library` revisions. Native source preserves custom pad primitives/graphics. Put mechanical models in project assets with `${KIPRJMOD}` paths via `model_import`, and attach STEP/STP/VRML models to footprints. Model offsets are mm, rotations degrees and scale unitless. GLB/STL/OBJ inspection/export does not imply KiCad can attach these as native footprint CAD models.
- `pcb_3d` renders a native assembly and reports missing/unassigned models. Inspect it for orientation, mounting and connector placement. `pcb_export` supports native PCB, fabrication files, placement/BOM and STEP/GLB/STL/VRML/BREP/PLY assemblies. Verify exported artifacts exist and reopen the native PCB in KiCad. Save explicitly; preview/commit never silently writes the board file.
- Background desktop PCB calls return pending IDs. Poll `operation_status`; never replay a timeout or switch to a different backend to repeat an edit. Use compact `pcb_inspect` filters or `since_revision`, not whole-document dumps. Read `help(topic:"pcb")` for recipes, then installed tool schemas for exact parameters.
