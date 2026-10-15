# Public release gate — 0.8.0

Source publication and public binary distribution are separate decisions. Source may be public while Windows downloads remain a draft. Version remains 0.8.0.

## Already verified locally

- 96 workspace tests; targeted release regressions after final worker changes.
- Release build, bridge interruption/upgrade/downgrade/repair, isolated installer maintenance and offline bundle operation.
- 3D tab/window/lighting, desktop-independent calculations, PNG and multi-view PDF exports.
- CRCs and SHA-256 of the local release artifacts. See RELEASE_080_VALIDATION.md.

These are local checks, not an independent audit or comprehensive clean-machine certification. Strict Clippy runs as a required-to-pass CI job alongside formatting and tests.

## Required before making Windows binaries public

- [ ] Inventory every redistributed native DLL and its exact version, license, required notices and source/rebuild location. The current backend preparation script copies all DLLs from the installed KiCad bin directory; KiCad's top-level COPYRIGHT.txt is not a per-DLL compliance inventory.
- [ ] Provide complete required notices and equivalent corresponding-source access next to binary downloads. Generic upstream homepages do not establish exact matching source for every runtime component. Verify redistribution rights for non-GPL runtime components too.
- [ ] Verify KiCad 10.0.6 source, exact Windows builder/dependency configuration, library license/exception and payload hashes. Retain the NewStroke GPL/MIT/OFL notices and Rust dependency licenses/source.
- [ ] Refresh source ZIP/documentation/checksums for the exact release commit. Existing local artifacts predate the public-repository documentation and must not be silently presented as a newly rebuilt, fully audited release.
- [ ] Run clean Windows installation and user-visible schematic/PCB/3D workflows, maintenance and native KiCad round trips. Record what remains unsupported.
- [ ] Review exposed MCP/CLI inputs, imported-file/resource limits and background-worker cancellation. Do not advertise an independent security audit unless one happened.
- [ ] Confirm release signing policy. Current builds are unsigned; label that accurately. Code signing is strongly recommended for general public distribution but is not falsely claimed as completed.
- [ ] Publish only after maintainer review. Use an early-access prerelease label; retain immutable hashes and matching source/notices. A website should link to canonical GitHub release assets, not maintain an unchecked second copy.

## Repository controls

Require PRs, one code-owner approval, required format/test checks and resolved conversations. Block force pushes/deletion and protect version tags. Restrict workflow token permissions, keep auto-merge disabled, require approval for fork workflows, enable private vulnerability reporting and dependency/secret protections where available. Never claim a setting is enabled without reading it back from GitHub.

Only Jeremy controls membership and release approval. Branch rules do not stop GPL-permitted forks and redistribution, guarantee personal safety or replace account two-factor authentication.
