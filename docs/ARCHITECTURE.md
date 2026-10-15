# Architecture

- `jenga-core`: semantic KiCad AST, document edits, atomic saves, a shared rendering scene, basic single-sheet ERC and exports. No GUI dependency.
- `jenga-libraries`: ordered library source discovery, KiCad global/project tables, custom libraries, indexing and inherited symbol resolution. No GUI dependency.
- `jenga-tools`: deterministic command engine, CLI, MCP stdio, authenticated live JSON transport, CPU screenshots, imported-library registration, batch edits and actor-attributed source journals. No inference/model dependency.
- `jenga-desktop`: native egui/eframe desktop app, projects/home, properties, canvas interactions, dialogs, history and persistent settings.
- `scripts`: resource preparation, validation/release builds and semantic version increments.
- `packaging/windows`: stable installer identity, in-place upgrades and downgrade protection.
- `resources/libraries`: generated upstream library payload and tracked provenance/license files. Large generated data is ignored by Git.

The core owns schematic changes and validation so external structured commands use the same model and operations as the UI. Unknown AST fields are retained; formatting and comments are not retained. Unsupported advanced objects remain in the file and are reported in the UI.

This release does not invoke KiCad to implement application features. KiCad CLI is used only as an external validation tool during development.
