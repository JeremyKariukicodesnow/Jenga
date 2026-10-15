# Versions and Windows releases

The workspace version in Cargo.toml is the single source of truth. All crates inherit it; Windows executable resources, About, installer and artifact names use the same version.

- Major (`1.0.0` → `2.0.0`): incompatible changes to supported Jenga interfaces/settings.
- Minor (`1.0.0` → `1.1.0`): new compatible features.
- Patch (`1.1.0` → `1.1.1`): fixes and small compatible iterations.

Before 1.0, minor releases may change preview interfaces; preserve existing projects and settings through explicit migration. Run `scripts/bump-version.ps1 -Part patch` (or minor/major), update CHANGELOG.md, then `scripts/build-release.ps1`. Commit/tag the validated release with `vMAJOR.MINOR.PATCH` when ready to publish.

Prepare symbols and footprints with `scripts/prepare-libraries.ps1`. The release script creates versioned Setup.exe, portable ZIP, matching source ZIP and SHA-256 checksums in `dist/VERSION/`.

Windows installs per user in `%LOCALAPPDATA%\Programs\Jenga`. Every version retains the same AppId, scope and architecture so running Setup.exe maintains the existing installation and Start-menu shortcut. A newer version offers Upgrade, the same version offers Repair, and an older version offers Downgrade. Existing installations skip Welcome, License and installation-choice pages and open the Ready maintenance page directly; fresh installations still show the license agreement. Unattended downgrades require `/ALLOWDOWNGRADE=1`. Running applications are asked to close. Settings live in the OS per-user configuration directory; projects live in user-selected directories. Neither is removed by maintenance or uninstall. Portable ZIPs must be extracted as a whole and do not upgrade an installed copy.

There is no online update feed yet. Do not claim automatic updates. Distribute source and third-party notices alongside binaries. Release binaries are currently unsigned; code signing requires a publisher certificate before a public signed release.

Install Inno Setup locally or supply `-IsccPath`. It is a build-time tool, not part of Jenga's engineering backend. Validate the Upgrade, Repair, interactive Downgrade and unattended downgrade opt-in paths before publishing. `scripts/verify-engineering.py` runs isolated API workflows, independent KiCad netlist checks when installed, and captures the project dashboard, symbol browser and existing symbol editor.
