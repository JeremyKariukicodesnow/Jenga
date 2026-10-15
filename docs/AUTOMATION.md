# Jenga external tools

Jenga has **no built-in AI interface, chat, model picker, API-key screen, model service, or inference dependency**. `jenga-cli.exe` exposes deterministic engineering operations for people, scripts and external assistants. MCP clients such as Codex, Claude Code or Cursor can call those operations through their own interfaces. No subscription or model credential is required by Jenga.

The installer and portable package include both `Jenga.exe` and `jenga-cli.exe`. All coordinates are millimetres unless a pointer request explicitly uses `space: "pixels"`. Both interfaces use the same native KiCad document model, library resolution, rendering and ERC code. No KiCad installation is required for normal operation. Symbols and footprints are bundled; 3D models are excluded.

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

Each response contains `id` and either `result` or `error`. Errors do not become terminal commands. Use `--workspace DIR` to resolve relative paths. A new tool process starts a new headless session; save before closing it.

## MCP client configuration

Add the following server entry in your external client's MCP configuration, adjusting the absolute executable and workspace paths:

```json
{
  "mcpServers": {
    "jenga": {
      "command": "C:\\Users\\YOUR_NAME\\AppData\\Local\\Programs\\Jenga\\jenga-cli.exe",
      "args": ["mcp", "--workspace", "C:\\YourProjects"]
    }
  }
}
```

This works headlessly while the desktop is closed. To control the running desktop instead, use:

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

Obtain the current revision from `inspect` and insert it as `params.expected_revision` in a real editing session. Wire routing in `connect_pins` uses outward pin stubs and an orthogonal midpoint route (optional `horizontal_first: false` chooses the other order). It does not avoid obstacles or automatically connect bare crossings; add a junction at an intended crossing and verify the route. For explicit routing, use `wire` with a `points` array.

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

Library authoring includes adding, editing, removing and renaming definitions in the complete native source. Preserve inheritance references when renaming symbols. Editing a library does not silently alter embedded schematic symbols: explicitly use `refresh_symbol` to update a target's definition, or `replace_symbol` with `library_id` and optional `unit`. Refresh affects all instances sharing that embedded ID. Replacement retains instance properties; review values, footprints and pin connections. Existing provenance is retained unless replacement metadata is supplied.

## Live desktop and pointer tools

The MCP server advertises 32 operations. `jenga_ui_action` can select an object, show the chooser, page settings, history or ERC, dismiss dialogs, or focus an ERC issue by index. `ui_state.dialogs` reports open dialogs. Focusing an issue navigates and highlights its electrical location; it does not automatically repair the circuit.

`jenga_ui_state` reports the open file, unsaved state, current tool, selected UUID, cursor, viewport, grid, library-indexing state and status. Queries work while the desktop is otherwise idle. Wait for `indexing: false` before placement. Changes become visible in the desktop and produce ordinary completion notifications. Document edits are undoable through the human interface; filesystem operations such as renaming and library writes use backups and history instead.

`jenga_pointer` uses the **same canvas placement code as the mouse**:

```json
{"at":[60,50],"tool":"symbol","library_id":"Device:R","phase":"click","actor":"ai:cursor"}
```

Supported tools: `select`, `symbol`, `wire`, `junction`, `no_connect`, `text`, `label`, `move`. Supported phases: `move`, `click`, `down`, `up`; drag uses primary-button Select. Right-click cancels placement. Text placement also needs `text`; symbol placement optionally accepts `unit`. Subsequent wire clicks must omit `tool` so they continue the active wire. `double: true` finishes a wire. Placement follows the current snapping grid. Modal dialogs must be dismissed before pointer control.

`jenga_set_view` accepts `center`, `scale` (pixels/mm), `grid`, `show_grid`, `fit`. A live `jenga_screenshot` request with `interface: true` captures the complete actual desktop window. The normal screenshot request renders just the schematic and worksheet offscreen; it also works headlessly.

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
