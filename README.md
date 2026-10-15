# Jenga

Jenga is an open-source, project-based schematic viewer and editor written in Rust, with KiCad file compatibility and a familiar, focused engineering interface.

Status: **0.5.1 preview**, Windows x64. See [user guide](docs/USER_GUIDE.md) for supported features and current limits.

## Run and build

Windows downloads are generated in `dist/0.5.1/`: run the versioned `Setup.exe`, or extract the portable ZIP and launch `Jenga/Jenga.exe`. Symbols and footprints are included; 3D models are excluded.

Development requires Rust and Windows MSVC build tools. Run `cargo run -p jenga-desktop` from this repository. Prepare libraries with `scripts/prepare-libraries.ps1` and build release artifacts with `scripts/build-release.ps1`. See [architecture](docs/ARCHITECTURE.md) and [versioning/upgrades](docs/RELEASING.md).

External tools: see [automation guide](docs/AUTOMATION.md) for CLI, MCP, JSON, screenshots, pin-aware wiring, imports, batch actions and attributed history. Jenga has no chat or model service.

Features implemented in the preview include project thumbnails, grouped library browsing, metadata search, symbol and 2D footprint previews, a placed-components palette, reliable selection and placement, property editing, drag/move/rotate, orthogonal wires, junctions, labels, text, no-connect markers, undo/redo, basic ERC and vector exports. Embedded symbol geometry and KiCad NewStroke lettering are used. KiCad-authored files are parsed and saved with unknown fields preserved, and real fixtures are tested for semantic round-trip preservation.

Legacy .sch import, full KiCad ERC, full advanced-object rendering and PCB editing remain future work. Compatibility is tested incrementally, not claimed for every historic KiCad version. Updates currently use the newer installer; no online updater is configured.

## Product direction

- Create and open projects, then create or open schematics within them.
- Offer a KiCad-style canvas, properties and hierarchy panels, familiar editing commands and shortcuts, with clear tool discovery and a focused interface.
- Support symbols, power symbols, wires, junctions, labels, text, properties, selection, movement, rotation, and undo/redo.
- Open KiCad schematics and save files that KiCad can reopen, preserving engineering data and visual appearance.
- Render actual KiCad symbol definitions faithfully, including pins, geometry, text, and orientation.
- Bundle selected core symbol and power libraries; support user-configured libraries and discovery of libraries from existing KiCad installations.
- Provide electrical rule checks and PDF/SVG export through Jenga's own Rust implementation.
- Add tested support for older KiCad formats, distinguishing import support from saving to an older format.
- Expose CLI, MCP and authenticated JSON tools for external assistants, with no built-in AI interface.

Jenga follows Option A: its own Rust interface and engineering core. KiCad's behavior, source, formats, and libraries may be studied, and appropriately licensed code may be reused. A KiCad runtime backend is not the selected architecture.

## Compatibility approach

Compatibility will be verified with real schematic fixtures and round-trip checks in both directions. Unsupported data should be preserved where possible and clearly reported. File versions will have explicit support boundaries; universal backward compatibility is not claimed. Library resolution should use schematic-embedded definitions where available so existing designs do not change appearance when installed libraries differ.

## License

Jenga's original code is licensed under **GPL-3.0-or-later**: GNU General Public License version 3, or (at your option) any later version. See [LICENSE](LICENSE).

Commercial use and sale are allowed. Distributing covered binaries requires compliance with the GPL, including corresponding source availability and recipients' rights to modify and redistribute the software.

Third-party code and assets retain their applicable licenses and copyright notices. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). KiCad compatibility alone does not make independently written code a derivative of KiCad; copying or adapting covered code introduces its license obligations.

Jenga is an independent project and is not affiliated with or endorsed by KiCad.

