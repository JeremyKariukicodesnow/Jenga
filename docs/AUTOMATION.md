# Jenga external tools

## 0.6.2 engineering workflows

`connect_block` routes a whole list atomically. Pin endpoints use exact physical numbers and UUIDs or reference/unit identifiers. The returned per-action results contain `action_index`; a failed applied batch reports the exact failing action and leaves the document unchanged. Prior batch routes become obstacles for subsequent routes. Optional `reserved_corridors:[{from:[x,y],to:[x,y]}]` reserves rectangular areas, and crossing costs favour clearer routes.

```json
{"method":"connect_block","params":{"expected_revision":"REVISION_FROM_INSPECT","connections":[{"from":{"reference":"R1","number":"2"},"to":{"reference":"C1","number":"1"}},{"from":{"reference":"C1","number":"2"},"to":{"reference":"U1","number":"3"}}],"dry_run":true}}
```

This example assumes those references and physical pins already exist. `dry_run:true` returns `preview_id`, `preview_image_path` (a fitted PNG of the proposed drawing), `base_revision`, proposed `changes` (including affected nets), `verification`, indexed `failures` and `can_commit`. It checks all actions and reports routing/connection/short failures without applying anything. Unnamed net merges outside stated expected connections also prevent commit. `preview_edit` provides the same workflow for any `actions` batch, including replacement and cleanup. Commit only a successful preview:

```json
{"method":"commit_preview","params":{"preview_id":"UUID_FROM_PREVIEW","operation_id":"CLIENT_GENERATED_UUID"}}
```

A different document, external file change, changed revision or failed preview rejects the commit. Commit is undoable and does not save implicitly. Call `save` explicitly after verification. Preview receipts live in Jenga's user data folder and contain the candidate native source.

Mutation responses include `operation_id` and `status`. Supply a UUID before starting a mutation to recover even if the transport loses the response. Poll `operation_status:{operation_id}` for `pending`, `completed` (with the retained result) or `failed` (with its error). Reusing the same ID and identical request returns the original result without repeating edits. Reusing it for a different request is rejected. Queued desktop jobs have durable receipts and cannot start after cancellation. A claimed operation still pending after process exit is indeterminate; inspect/recover the document before issuing a new ID. Operation completion does not imply that unsaved edits have been saved to disk.

```json
{"method":"edit","params":{"expected_revision":"REVISION_FROM_INSPECT","actions":[{"op":"replace_symbol","reference":"U1","library_id":"MyLibrary:NewPart","pin_map":{"1":"7","2":"8"}}]}}
```

Replacement/refresh first matches identical numbers, then unique nonempty pin names, or uses the supplied old-to-new `pin_map`. Ambiguous, missing or many-to-one mappings fail atomically. All old pins require an unambiguous mapping, including unused pins. Existing wires/labels are preserved through routed links to moved pins; no-connect markers follow their mapped pins. Splits and shorts reject the edit. Refreshing one instance isolates its embedding so other instances retain their geometry. Instance value and footprint properties remain for the caller to review.

`inspect` accepts `references`, `uuids`, `kinds`, `include_pins:false`, `nets:true`, and `net_names`. `since_revision` returns added/changed/removed objects and affected nets from the current document's recorded history; an unknown revision asks for a full inspect. These selectors work in the desktop's read-only snapshot as well as headless mode.

```json
{"method":"verify","params":{"expected_connections":[{"from":{"reference":"R1","number":"2"},"to":{"reference":"C1","number":"1"}}],"forbidden_connections":[{"from":{"reference":"U1","number":"1"},"to":{"reference":"U1","number":"3"}}]}}
```

One verification returns ERC findings, intended/forbidden connections, exact dangling endpoints and native KiCad roundtrip/missing-symbol warnings. It names missing pins without aborting other checks. Analysis is single-sheet; native checks are not external KiCad CLI validation or hardware certification. Independent drawings have separate electrical scope; hierarchical child pages connect through explicit native sheet pins and require KiCad for cross-sheet analysis.

`edit` action `{op:"cleanup"}` removes genuinely unattached junctions, duplicate/zero-length wires and safe collinear joins. It retains connected dots, branches, labels and pin anchors and checks electrical membership before applying. Use a preview to review the removals.

Home's **Archived projects** section lists hidden projects and recoverable deletions with Restore actions. API equivalents are `list_archived_projects` and `restore_project` with either a hidden project's `path` or its `restore_manifest`. Recovery refuses occupied destinations and rolls back partial moves. Page recovery manifests have separate parent-sheet metadata and are not listed as projects.

Jenga has **no built-in AI interface, chat, model picker, API-key screen, model service, or inference dependency**. `jenga-cli.exe` exposes deterministic engineering operations for people, scripts and external assistants. MCP clients such as Codex, Claude Code or Cursor can call those operations through their own interfaces. No subscription or model credential is required by Jenga.

The installer and portable package include both `Jenga.exe` and `jenga-cli.exe`. All coordinates are millimetres unless a pointer request explicitly uses `space: "pixels"`. Both interfaces use the same native KiCad document model, library resolution, rendering and ERC code. No KiCad installation is required for normal operation. Symbols and footprints are bundled; 3D models are excluded.

## Discover capabilities

Call `jenga_help` with optional `topic`: `overview`, `tools`, `editing`, `libraries`, `connections`, `limits`, `all`. No document is required. Topic `tools` returns complete MCP descriptions and argument schemas, supported coordinates and save semantics. MCP initialize also points assistants to this help tool. CLI equivalents:

```powershell
jenga-cli help --json
jenga-cli help --topic editing
```

Use the human interface's Home → Connect Assistant for reviewed copyable configuration, user-wide registration, an exported connection kit and an optional installable `jenga-design` skill. Each client has its own registry; user-wide registration applies to that client across workspaces, not every assistant automatically. Model accounts/credentials remain in the external client.

```powershell
jenga-cli setup --client Codex --workspace C:\Projects
jenga-cli setup --client Cursor --workspace C:\Projects --install
jenga-cli setup --client "Claude Code" --workspace C:\Projects --install --skill
jenga-cli setup --client Antigravity --mode live --workspace C:\Projects
```

Without `--install`, setup prints configuration only. `--skill` explicitly installs the bundled skill for supported clients. Installation merges one Jenga entry, backs up existing configuration, validates JSON/TOML and updates an existing Jenga CLI entry while refusing entries owned by another command. It does not enable auto-approval or alter other tools. Restart/refresh the external client afterward. Default user-wide paths: Codex `.codex/config.toml`, Claude Code `.claude.json`, Cursor `.cursor/mcp.json`, Antigravity `.gemini/config/mcp_config.json`. For custom config homes or other clients, export/copy the entry into their chosen config instead.

Official setup references: [Codex](https://developers.openai.com/codex/mcp), [Claude Code](https://code.claude.com/docs/en/mcp), [Cursor](https://cursor.com/docs/mcp), [Antigravity](https://www.antigravity.google/docs/mcp). These instructions were checked on 2026-10-12; client settings may evolve.

## CLI and JSON

```powershell
& "$env:LOCALAPPDATA\Programs\Jenga\jenga-cli.exe" --help
jenga-cli inspect --file circuit.kicad_sch
jenga-cli search_symbols --query resistor
jenga-cli erc --file circuit.kicad_sch
jenga-cli screenshot --file circuit.kicad_sch --output circuit.png
jenga-cli export --file circuit.kicad_sch --output circuit.pdf --format pdf
jenga-cli run --file circuit.kicad_sch --request edits.json --save
```

Use the full executable path if it is not on your PATH. `run` accepts a JSON file or a JSON object string; it does not save unless `--save` or a `save` request is used. `json` keeps a document and undo history open and accepts one newline-delimited request at a time:

```json
{"id":1,"method":"open","params":{"path":"circuit.kicad_sch"}}
{"id":2,"method":"inspect","params":{}}
{"id":3,"method":"erc","params":{}}
{"id":4,"method":"save","params":{"actor":"ai:codex"}}
```

Each response contains `id` and either `result` or `error`. Errors do not become terminal commands. Use `--workspace DIR` to resolve relative paths. The automatic backend follows the running app when available. A new headless process starts its own session; save before closing it. `--headless` forces isolated disk operation. The workspace is only a default for relative paths; absolute project/save paths can be anywhere you choose.

## MCP client configuration

Add the following server entry in your external client's MCP configuration, adjusting the absolute executable and workspace paths:

```json
{
  "mcpServers": {
    "jenga": {
      "command": "C:\\Users\\YOUR_NAME\\AppData\\Local\\Programs\\Jenga\\jenga-cli.exe",
      "args": ["mcp", "--workspace", "C:\\YourProjects", "--connect", "auto"]
    }
  }
}
```

This single connection works with the desktop open or closed. It uses the running window or a headless session automatically. Save before switching. To choose the same automatic connection without a workspace override, use:

```json
{"command":"C:\\absolute\\path\\jenga-cli.exe","args":["mcp","--connect","auto"]}
```

`jenga-cli sessions` lists responding desktop process IDs without printing authentication tokens. If several Jenga windows are open, use `--connect PID` instead of `auto`. `auto` chooses the newest responding session. A CLI connection does not start the desktop automatically.

MCP uses stdio JSON-RPC and negotiates protocol `2025-11-25` or `2025-06-18`. Standard output contains only protocol messages. Client configuration formats differ; the entry above is an example, not a claim that Jenga has modified your client settings. No external client account has been connected automatically.

## Inspect, edit, verify

1. Open a project or schematic using `jenga_open`, or create one with `jenga_create_project` (`name`, optional `folder`) / `jenga_new` (optional `path`).
2. Call `jenga_inspect`. It returns stable object UUIDs, references, units, values, footprints, positions, pin numbers and electrical types, wire vertices, text, object bounds, native page metadata and a content `revision`.
3. Search with `jenga_search_symbols` (`query`, optional `library`, `limit`). Descriptions and inherited keywords are searched, so “resistor” finds `Device:R`. `jenga_get_symbol` and `jenga_get_footprint` return the resolved native definition and preview bounds.
4. Apply a `jenga_edit` request with `expected_revision` and an `actions` array. The entire document edit is validated on a clone; if any action fails, none of its document changes are committed.
5. Run `jenga_erc`; inspect the suggested repairs and locations. Generate `jenga_screenshot` or `jenga_export` to check the colored rendered result.
6. Save explicitly with `jenga_save`. Existing files receive a `.jenga-backup` before replacement.

Example `edits.json`:

```json
{
  "method": "edit",
  "params": {
    "actor": "ai:codex",
    "actions": [
      {"op":"add_symbol","library_id":"Device:R","at":[60,50],"properties":{"Value":"10k","Footprint":"Resistor_SMD:R_0805_2012Metric"}},
      {"op":"add_symbol","library_id":"Device:C","at":[90,50],"properties":{"Value":"100n"}},
      {"op":"connect_pins","from":{"reference":"R1","number":"2"},"to":{"reference":"C1","number":"1"}},
      {"op":"no_connect","pin":{"reference":"R1","number":"1"}},
      {"op":"text","text":"Filter stage","at":[60,35]},
      {"op":"title_block","fields":{"title":"Filter","company":"Jenga","revision":"A","comments":["Engineering review required"]}}
    ]
  }
}
```

Obtain the current revision from `inspect` and insert it as `params.expected_revision` in a real editing session. Wire routing in `connect_pins` uses exact pin endpoints and outward stubs, searches simple orthogonal detours around other symbols, and refuses unsafe routes. It returns route coordinates. Bare crossings stay separate; add a junction only for an intended branch. Call `connectivity` to verify actual pin-to-net membership; zero ERC findings alone are insufficient. For explicit routing, use `wire` with a `points` array.

Other edit actions:

| Operation | Fields |
|---|---|
| `add_symbol` | `library_id`, `at`, optional `unit`, `rotation` (multiples of 90), `properties` |
| `move` / `translate` | Object `uuid` or `reference` + optional `unit`; `at` / `delta` |
| `rotate` / `delete` | Object `uuid` or unique `reference` + optional `unit` |
| `set_property` | Symbol target, `key`, `value` |
| `text`, `label`, `global_label`, `hierarchical_label` | `text`, `at` |
| `set_text` | Text/label object target, `text` |
| `junction` / `no_connect` | `at`, or `pin` with target and `number` |
| `title_block` | `fields` with `title`, `company`, `date`, `revision`, `comments` (up to nine) |
| `paper` | `name`, optional `portrait`; `size_mm: [width,height]` for `User` |

Use UUIDs for wires and other non-symbol objects. Units of one physical part share a reference. `jenga_undo` and `jenga_redo` operate within the current headless or live session. `jenga_read_file` reads UTF-8 project data; `jenga_list_projects` lists native schematic/project files recursively.

## Rename, create and author libraries

`jenga_rename` accepts `name`, `kind: "schematic"` or `"project"`, and optionally `move_folder: true`. Save first. Project renames update the native project filename and schematic instance references, preserve UUIDs, reject destination collisions and roll back filesystem failures. Moving the folder is opt-in and requires it to have the old project name. The File menu exposes the same operation.

`jenga_create_schematic` saves a new `path` without overwriting an existing file. A child schematic in a project folder uses that project's name. `jenga_close` requires a clean document or explicit `discard_changes: true`. Rename a component with an `edit` action `set_property` using `key: "Reference"`; use its stable UUID afterward.

| Library operation | Purpose |
|---|---|
| `list_libraries` | Source paths, discovery warnings, symbol and footprint counts |
| `search_footprints` | Search all available 2D footprint IDs |
| `create_library` | Empty registered project library; `name`, `kind: "symbols"` or `"footprints"` |
| `read_library` | Native library/footprint `path`, source and content revision |
| `write_library` | Validated native `source`, library alias `name`, and `expected_revision` when updating; optional license/attribution |
| `preview_symbol` / `preview_footprint` | Colored fitted PNG; `id`, output `path`, optional width; symbols accept `unit` |

Library authoring includes adding, editing, removing and renaming definitions in the complete native source. Preserve inheritance references when renaming symbols. Editing a library does not silently alter embedded schematic symbols: explicitly use `refresh_symbol` to update a target's definition, or `replace_symbol` with `library_id` and optional `unit`. Refresh and replacement isolate the selected instance embedding, map old pins to new pins, preserve attached wiring and reject ambiguous mappings or changed net membership. Use pin_map:{"old_number":"new_number"} when numbers/names cannot identify one new pin. Replacement retains instance properties; review values and footprints. Existing provenance is retained unless replacement metadata is supplied.

## Live desktop and pointer tools

The MCP server advertises 33 operations. `jenga_ui_action` can select an object, show the chooser, page settings, history or ERC, dismiss dialogs, or focus an ERC issue by index. `ui_state.dialogs` reports open dialogs. Focusing an issue navigates and highlights its electrical location; it does not automatically repair the circuit.

`jenga_ui_state` reports the open file, unsaved state, current tool, selected UUID, cursor, viewport, grid, library-indexing state and status. Queries work while the desktop is otherwise idle. Wait for `indexing: false` before placement. Changes become visible in the desktop and produce ordinary completion notifications. Document edits are undoable through the human interface; filesystem operations such as renaming and library writes use backups and history instead.

`jenga_pointer` uses the **same canvas placement code as the mouse**:

```json
{"at":[60,50],"tool":"symbol","library_id":"Device:R","phase":"click","actor":"ai:cursor"}
```

Supported tools: `select`, `symbol`, `wire`, `junction`, `no_connect`, `text`, `label`, `move`. Supported phases: `move`, `click`, `down`, `up`; drag uses primary-button Select. Right-click cancels placement. Text placement also needs `text`; symbol placement optionally accepts `unit`. Subsequent wire clicks must omit `tool` so they continue the active wire. `double: true` finishes a wire. Placement follows the current snapping grid. Modal dialogs must be dismissed before pointer control.

`jenga_set_view` accepts `center`, `scale` (pixels/mm), `grid`, `show_grid`, `fit`. `jenga_export` supports `pdf`, `svg`, `png`, `csv`, `xlsx` and `kicad_sch`. XLSX is a real Excel workbook with typed text cells, a frozen header and filters; it is not a renamed CSV. Native export keeps the active document path unchanged.

A live `jenga_screenshot` request with `interface: true` captures the complete actual desktop window. The normal screenshot request renders just the schematic and worksheet offscreen; it also works headlessly.

The desktop opens an authenticated loopback-only JSON socket. Discovery files live in `%LOCALAPPDATA%\Jenga\sessions`; each contains a random session token. A raw local request is one JSON line: `{"token":"...","id":"...","method":"inspect","params":{}}`. The CLI handles discovery and authentication. Tokens are not AI credentials and are not sent to a model provider. The channel is not a public web server. Timed-out writes must be checked using `inspect` before retrying because an accepted operation may already have completed.

## Imports and batch operations

An external assistant may use its own web search to find a native library URL, then call `jenga_import_library` with `name`, `url` (HTTPS), `license`, `attribution`. A local `path` is also supported. Native symbol libraries and 2D footprint files are validated as data, copied into the saved project, registered in `sym-lib-table` / `fp-lib-table`, and accompanied by a provenance JSON file. Existing items require the current `expected_revision` to be overwritten, and receive a backup. Unknown license metadata remains unknown; Jenga does not silently grant redistribution rights. Downloads use the Windows `curl.exe` utility, have a timeout and size cap, and never execute downloaded content. There is no integrated web search/model service inside Jenga.

```powershell
jenga-cli batch --folder C:\Projects --request edits.json --dry-run
jenga-cli batch --folder C:\Projects --request edits.json
```

Batch edits are staged and validated before saving. Dry runs create no schematic changes or journal entries. Reports contain each file's revision and ERC results. Each saved file is backed up individually; this is **not a filesystem-wide transaction**. If a save fails midway, earlier files may already be saved. A saved request that contains an `expected_revision` usually targets one file; omit that field for a deliberately shared batch recipe. Batch requests accept document edit actions, not arbitrary operating-system commands.

## History and current limits

Changes carry an `actor` such as `user`, `ai:codex`, `ai:claude`, or `external`. User editing is coalesced and journaled; external transactions record their exact before/after native source and revisions. Journals are plain JSONL in `.jenga/history` alongside the schematic. Unsaved documents use a temporary local journal, copied into the project on first save. This attribution is a client-supplied label, not proof of identity. Keep the journal directory with the project if you want to retain history. `jenga_history` returns summaries; full source snapshots remain available in the journal.

Jenga currently implements single-sheet electrical checks, not KiCad's entire hierarchical rule matrix. Custom `.kicad_wks` drawing-sheet templates are not rendered. Legacy `.sch`, PCB editing, 3D, obstacle-avoiding autorouting and built-in model inference are outside this release. Imported native data that Jenga does not understand is preserved in the schematic AST. Offscreen PNGs are engineering renders, not fake UI screenshots.

Protocol reference: [official MCP 2025-11-25 schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/schema/2025-11-25/schema.ts).


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


### 0.5.1 clipboard, grid and skill tools

`view_skill` returns the bundled versioned skill text. `export_skill` takes `path` and writes SKILL.md without overwriting an existing file. Neither needs an open app or circuit. CLI: `jenga-cli skill` to read; `jenga-cli skill --output C:/AssistantSkills/jenga-design/SKILL.md` to export. MCP names are `jenga_view_skill` and `jenga_export_skill`.

`copy_objects` takes optional `uuids` (omit for every supported object), returning native KiCad clipboard `text`. `paste_objects` takes that `text`, optional `at:[x,y]`, `expected_revision` and `actor`. The paste block anchor uses the source minimum rounded down to the 1.27 mm grid; offsets preserve relative geometry. The default offset is 10.16 mm. Paste is undoable and requires explicit save. CLI/JSON can call the same methods using `run --request request.json`; the tools return text, not the OS clipboard.

In the UI click an object then Ctrl+C, or Ctrl+A then Ctrl+C for the whole schematic. Ctrl+X cuts, Ctrl+V pastes at the last canvas cursor. Text editing retains ordinary text copy/paste. Native clipboard fragments contain embedded symbols, wires, labels, junctions, no-connects and text. Paste supports sibling expressions produced by KiCad and complete flat schematic text. It creates fresh UUIDs, reannotates references, keeps units of a physical part together, and isolates conflicting embedded definitions. It does not flatten hierarchical sheets, buses, groups, images or arbitrary unsupported types. Those are rejected; use native file opening to preserve unsupported data. Named labels retain names: review any intentional or unintended net-name joins after paste. Clipboard is text; larger fragments can hit the transport request limit.

Use 1.27 mm positions for new electrical geometry. Routing eliminates redundant collinear vertices and snaps detour axes outward around obstacles; it never snaps imported pin endpoints independently. ERC grid warnings use KiCad's default 1.27 mm connection grid (custom-grid designs may deliberately trigger them). Zero findings is not simulation or build certification.

## 0.6 project workspace and engineering tools

Home project cards have icon actions for Project, Rename, Archive from Home (keeps files), and recoverable Delete. Project workspace provides page previews, creation, symbol editing and selected-page export. Save As and Delete are available as icons in the schematic toolbar. Home and Project titles remain dark during animated reveals, with moving traces, dot patterns and rotating wireframe blocks; Reduce Motion remains available.

`project_info` returns the project folder, native project file and ordered independent/hierarchical pages. `create_page` accepts `name`, optional `hierarchical` (default true), `at`, `page_number`, `expected_revision` and `actor`. Save before page creation. It creates `.kicad_sch` files and a native child-sheet reference, not an app-only page format. Open the new file explicitly to edit it. Full cross-sheet ERC is still outside Jenga's electrical-analysis scope.

`export_pages` accepts `path`, `format` and optional `pages` (exact page names or absolute file paths). Omit pages for the full project. PDF exports one document; SVG/PNG/native exports one file per page to the destination folder. Selection is checked and exported in project order.

Text creation accepts `size_mm` and `align`. Edit operations `format_text` and `format_property` (add `key` for a symbol field) accept `size_mm`, `align:left|center|right`, `at`, `angle`, `upright`, `bold`, `italic` and `hidden`. Native effects and unknown fields are retained. `annotate` fills unassigned single-unit references while preserving assigned references by default; `reset:true` explicitly reannotates existing references. Ambiguous unassigned multi-unit symbols require explicit physical grouping first.

`connect_pins` avoids unrelated pins, labels, junctions and wire endpoints, uses a bounded path search when simple routes fail, and rejects unrelated net merges. Routing may still fail in a congested drawing; improve layout rather than forcing a short. `verify_connections` accepts expected_connections/forbidden_connections arrays, each entry `{from:{reference,number,unit?},to:{reference,number,unit?}}` (UUIDs also work). The same arrays on `edit` make intent checking transactional. Full visible wiring is the default skill workflow; cross-page links and user-requested label-based drawings are exceptions.

`save_symbol` accepts library/name, scope (`project` or `personal`), prefix, description, footprint ID, body dimensions and structured pins: `{number,name,kind,at:[x,y],angle,unit,length_mm}`. Optional graphics uses native KiCad primitives; optional `native_source` preserves an existing full symbol instead. `copy_symbol` accepts source `id`, destination `library`, optional scope and provenance. It keeps units, graphics and footprint links. Existing target libraries need expected_revision from read_library. Personal libraries are in `%APPDATA%/Jenga/libraries`, with native library tables, and can be searched/used from another project. Project libraries retain `${KIPRJMOD}` relative identity and take precedence over personal libraries. Native model references remain in footprints; get_footprint exposes models_3d. No PCB/3D assembly placement is implied.

Assistants can read the same bundled instructions using MCP `view_skill`, JSON `view_skill`, or CLI `skill`; `help` topic tools describes current schemas. Export/download using export_skill or CLI skill --output. Keep professional title blocks, destination annotations, visible power/return wiring and verification evidence as described by the skill.

The installer keeps its AppId and existing installation directory. Newer/same/older packages offer Update/Repair/Downgrade. Silent downgrades require /ALLOWDOWNGRADE=1. Projects, personal libraries and client arguments are retained. Installation runs `jenga-cli repair-connections`, which backs up and repairs existing Jenga command paths in supported clients without creating unrelated registrations or changing workspace/mode arguments. The CLI setup resolver prefers the registered installed executable instead of a versioned portable folder. Restart an affected assistant once if its cached process was running during replacement. No model accounts or keys are modified.

### Windows requests and assistant setup

PowerShell can strip inline JSON quotes. Prefer a UTF-8 JSON file, or pipe `ConvertTo-Json -Depth 20` output into `jenga-cli run --request -`. UTF-8 BOM files are accepted. Antigravity supports `setup --client Antigravity --install --skill`; its skill is stored in `.gemini/config/skills/jenga-design`. Generic clients can copy the stdio snippet and choose `--skill-dir ABSOLUTE_SKILLS_FOLDER`. Hermes/OpenClaw integrations depend on the host's support for stdio MCP or CLI execution; Jenga does not assume their config paths.

Desktop discovery remains available while a request is pending. UI state checks fail promptly if the UI is modal-blocked; expired queued requests are cancelled before execution. Never replay an uncertain mutation: inspect revision/history. For headless reading, choose the saved file explicitly and preserve unsaved desktop changes.

### Symbol graphics and labels

Draw lines, polylines, polygons, rectangles, circles, three-point arcs, and text labels. Added shapes can be selected, moved or deleted. Reference prefix, value label, description, footprint ID, pin names and numbers are editable. Pins have electrical types, graphic styles, units, length and hidden state. Imported native graphics remain editable through the source view; imported label formatting and extra pin metadata survive pin-table edits.

### v0.6.1 robust diagnostics

Every CLI subcommand accepts --help and exits (including mcp/json/run). The MCP endpoint is stdio JSON-RPC, not HTTP; the desktop endpoint is authenticated newline JSON, also not HTTP. Never kill an unknown desktop to diagnose it. Native MCP supports protocol versions 2024-11-05, 2025-03-26, 2025-06-18 and 2025-11-25. Stdout carries protocol messages only.

`jenga-cli connectivity|diagnose|project_info --file FILE --headless` provides direct diagnostics; `run --request -` accepts piped JSON. `diagnose` returns erc.issues and connectivity.nets. ERC's legacy response is an array, and connectivity uses names[]/pins[]/wire_uuids[]. All connectivity revisions now match inspect revisions. `trace_connection` inspects actual pin membership through wires, labels and T contacts. Differently named net merges are rejected transactionally unless allow_named_net_merges:true declares intentional aliases.

Live inspection/ERC/connectivity run against a published immutable document snapshot rather than waiting behind a native dialog. ui_state/diagnose report snapshot_age_ms. Writes stay on the UI thread, stale/inactive UI writes are refused, and no uncertain write is replayed headlessly. Read fresh revision/history, then save explicitly before switching to another process or disk inspection.

create_project's folder is the parent unless its final directory already matches name; in that case it is the exact destination, avoiding nested name/name folders. Use the returned path rather than shell-moving project files.

manage_symbol handles revision-guarded rename/delete/export of custom project/personal symbols; delete_page removes a native child reference and archives a non-root page. Installed libraries and embedded schematic definitions are preserved by these workflows. Call help/tools for exact schemas.
