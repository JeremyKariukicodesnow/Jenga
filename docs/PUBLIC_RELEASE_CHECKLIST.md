# Early-access publication record and future release checks

Jenga 0.8.0 is public as an unsigned **early-access prerelease**. The maintainer requested publication of the preserved Windows installers and historical releases. Version remains 0.8.0. This is not a production-readiness or independent audit claim.

## Published 0.8.0 evidence

- Installer SHA-256: `84e4286a82d5e2d23a97e648ca9ac13c00754877fcebe968615e7dc2fcb49c71`.
- Matching original source SHA-256: `00beb8e33f25d415a6157d4961466a2e5488594876e4d867f62ff4419e705b89`. All 75 checked Rust/build source files match the existing v0.8.0 tag. The source ZIP retains vendored Rust dependency sources/licenses. These preserved artifacts were not rebuilt from newer main.
- Native supplement SHA-256: `b3db98f4b0bda5a8932b758a3b693f190c736e330a08397b34fec4b0dea21de0`. It maps all 209 native files to 40 components, retains upstream notice material and provides pinned source/build access. Native KiCad commit: `caf7377e9cb6fa1535ec3596dcb8c99bf44a996e`; both vcpkg baselines/overrides and selected port patches are recorded. Per-component source endpoints were checked; ngspice's exact commit was fetched, and original SDK/GNU archives used for notices were verified against pinned SHA-512 where recorded.
- Uploaded installer/source/supplement hashes matched GitHub's recorded SHA-256 digests. Anonymous EXE download returned HTTP 206 and the Windows `MZ` signature. ZIP CRCs and saved artifact checksums passed.
- The original local 0.8 validation includes 96 tests, isolated installation/upgrade/downgrade/repair, offline bundle operation and schematic/PCB/3D exports. See RELEASE_080_VALIDATION.md for scope and limits. New main's strict Clippy, Windows tests and release build also passed, but are not a claim that the frozen installer uses main's later dependency revisions.
- Historical 0.1.0–0.6.2 releases have restored source tags and their original EXE/portable/source/checksum assets. Historical 0.7 releases use the same native backend as 0.8; they are published only after their corresponding artifacts and supplement uploads are verified. Do not claim a historical installer was freshly rebuilt or independently security-audited.

The native source/notice supplement is part of the release materials. Preserve it when redistributing. Source/access URLs are next to the binaries; maintain their availability or provide a matching mirror if an upstream disappears. Microsoft CRT/UCRT/SDK components keep their separate terms and are not relicensed as Jenga GPL source. The component labels do not replace the upstream license texts.

## Future releases

- [ ] Reinventory a changed native backend; identify every component's version, notices, redistribution terms and matching source/build access. Current notice coverage applies only to its recorded native hashes.
- [ ] Run `scripts/verify-native-notices.ps1` against the actual payload; packaging refuses altered, missing or unmapped native DLL/EXE files. The fixture tests cover changed/unmapped file rejection and real notice archive coverage. Metadata-only CI does not verify ignored native runtime bytes.
- [ ] Build a new version from its intended source commit. Include matching Jenga/Rust source, native notices/source directions, libraries' license/exception, NewStroke notices and hashes. Never overwrite a published artifact or silently move its tag.
- [ ] Verify anonymous EXE downloads, matching remote digests and the permanent page's selected version. Publish as prerelease until stable release criteria are met.
- [ ] Expand clean-Windows, native round-trip and import/resource-limit checks. Keep cancellation, MCP/CLI boundaries and project recovery under review. No independent security audit has occurred.
- [ ] Establish code signing and a broader supported-machine matrix before advertising general production readiness. Current historical/0.8 installers are unsigned.

Main requires pull requests, code-owner review, resolved conversations, Windows checks and strict Clippy. Admin/owner recovery powers remain documented in GOVERNANCE.md; no independent human review is implied by owner-authorized setup merges. Only Jeremy controls collaborators and release approval. These controls do not prevent GPL-permitted forks or guarantee personal/legal safety.
