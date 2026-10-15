# Jenga 0.8.0 validation

Validated on Windows x64, 15 October 2026.

- `cargo test --workspace --locked`: 96 tests passed. Nine release-specific tests were rerun after the final background-worker receipt change and passed.
- `cargo build --release --locked`: passed.
- `scripts/verify-release-080.py`: interrupted edit was not replayed; the same MCP bridge survived real 0.7.1/0.8.0 worker upgrade, downgrade and repair.
- `scripts/verify-installer-080.py`: isolated fresh install, upgrade, repair and downgrade passed; connection arguments, unrelated server settings and a personal-library marker were preserved.
- `scripts/verify-viewer-080.py`: assembly tab, adjustable lighting, separate window and exports passed. A CLI-owned export completed after its desktop process closed. Export brightness changed the rendered pixels. The terminal project was opened for inspection without saving modifications.
- Visually inspected the bright assembly screenshot after moving lighting controls onto their own row; controls fit and the assembly is centred.
- Release CLI exported a three-page assembly PDF with isometric/top/bottom views and adjusted lighting.
- Portable archive CRC verification passed for all 23,444 entries, including after the changelog update. Maximum installer compression proved impractically slow on the large model library; the release uses fast compression instead. No installer-size reduction is claimed.
- `scripts/verify-offline-080.py`: packaged CLI loaded a symbol, footprint and STEP model with installed KiCad discovery, registered libraries and KiCad environment overrides disabled. Only bundled library sources were discovered; native STEP processing succeeded through the bundled backend.

Regression coverage includes full visible schematic wiring, exact-part voltage/current findings, stale-preview rejection, cancellation receipts, native clearance inheritance, PCB clipboard net/reference handling, asset provenance and project transfer with local model dependencies.

## Practical limits

- Existing assistants need one restart to adopt the stable bridge. Hosts without dynamic MCP tool-list refresh may need a restart after tool changes; no replacement configuration snippet should be necessary.
- Background calculations are snapshots. Applying a result still requires a valid revision and explicit commit/save. Process interruption is reported and never authorizes automatic mutation replay.
- Complete hierarchical designs use whole-project import. Individual schematic import is independent and rejects hierarchy rather than silently dropping child sheets.
- Clipboard exchange supports common native objects; unsupported objects are reported. This does not claim every KiCad version or clipboard object has been tested through the KiCad GUI.
- Native DRC remains necessary for complex KiCad rules and geometry. Evidence records depend on supplied manufacturer data; CAD checks do not certify electrical, thermal, RF, mechanical or bench-tested performance.
- No controlled before/after RAM or speed benchmark was performed. The changes reduce specific indexing, snapshot and geometry work; no percentage improvement is claimed.
