# Download links, websites and videos

## Current availability

Jenga 0.8.0 is a **public early-access prerelease** with a full Windows EXE, matching preserved source, SHA-256 checksums and native notice/source supplement. Anonymous EXE download was verified. Historical releases are available separately. The permanent page selects the highest published installer; GitHub’s `/latest` routes select stable releases only. See PUBLIC_RELEASE_CHECKLIST.md for evidence and future checks.

## Permanent links

**Recommended shareable installer link:** https://jeremykariukicodesnow.github.io/Jenga/download/?start=1

This repository-hosted page checks public releases, selects the highest version including prereleases and redirects to its exact EXE. The page without `?start=1` shows the version and download choices first. It never offers drafts and never silently substitutes an older installer when the newest one is incomplete. Older release assets remain untouched. If the API is unavailable/rate-limited, it links to the release history. It is a browser download helper, not an in-app updater.

**Older versions:** https://github.com/JeremyKariukicodesnow/Jenga/releases. Pick a release/tag and its installer; that version-specific asset stays attached to that release. Preserved 0.1.0–0.6.2 historical early-access installers, portable ZIPs and matching source are public. They are schematic-only releases; read each release before downgrading.

The GitHub-native links below refer to **stable** releases only:

| Purpose | URL |
| --- | --- |
| All releases, including published previews | https://github.com/JeremyKariukicodesnow/Jenga/releases |
| Latest public stable release page | https://github.com/JeremyKariukicodesnow/Jenga/releases/latest |
| Direct latest stable Windows installer | https://github.com/JeremyKariukicodesnow/Jenga/releases/latest/download/Jenga-windows-x64-Setup.exe |
| Direct latest stable portable ZIP | https://github.com/JeremyKariukicodesnow/Jenga/releases/latest/download/Jenga-windows-x64-portable.zip |
| Latest stable source bundle | https://github.com/JeremyKariukicodesnow/Jenga/releases/latest/download/Jenga-source.zip |
| Latest stable checksums | https://github.com/JeremyKariukicodesnow/Jenga/releases/latest/download/SHA256SUMS |

GitHub's `latest` endpoint does not select drafts or prereleases. A published early-access prerelease is available from **All releases**, with a version-specific direct URL such as `https://github.com/JeremyKariukicodesnow/Jenga/releases/download/v0.8.0/Jenga-windows-x64-Setup.exe`. The 0.8.0 canonical EXE URL is public. Source and native notices are attached on its release page. Do not relabel a preview as stable just to make a link work.

The fixed asset filename is deliberate: versioned filenames cannot share the same `/latest/download/` URL across versions. `prepare-download-aliases.ps1` creates byte-identical canonical aliases alongside the versioned files and includes both names in SHA256SUMS. Hard links conserve local build space where supported; other volumes use copies. Stale aliases are rejected rather than silently replaced. Building aliases never uploads/publishes a release or clears its review gate.

After the release checklist is complete, upload the canonical assets, retained versioned artifacts and SHA256SUMS to the same release as their matching source/notices. Maintain versioned artifacts for auditability, verify anonymous downloads and hashes, and never replace published content silently. Draft-only assets and URLs are not usable by ordinary website visitors. The published 0.8.0 EXE/source bytes are the preserved original artifacts; later repository-main changes are not silently presented as a rebuild of that release.

## On your website

For the newest published build (including previews), link your website button to the shared resolver:

```html
<a href="https://jeremykariukicodesnow.github.io/Jenga/download/?start=1">
  Download Jenga for Windows (x64)
</a>
<a href="https://github.com/JeremyKariukicodesnow/Jenga/releases">All releases and previews</a>
```

The resolver shows an honest unavailable/status message before public installers exist; it cannot bypass a draft. For websites that want stable builds only, use the GitHub-native latest-stable EXE link in the table instead. Version-specific links remain the right choice when recommending an exact older build. These links work on any website or in a message; they do not add an automatic updater to Jenga.

GitHub reference: https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases

## Hosting and verification

The download helper is static HTML/CSS/JavaScript hosted from this repository's `/docs` directory by GitHub Pages. It uses anonymous, read-only GitHub release API requests; no credentials, analytics or upload endpoint are embedded. Only exact installer filenames and HTTPS GitHub download URLs for this repository/tag are accepted. It scans up to 1,000 release records and fails to manual selection if it cannot safely determine the newest version.

Run `node --test scripts/test-release-links.mjs` and `pwsh -File scripts/test-download-aliases.ps1` before changing selection or naming. Keep the GitHub Pages branch set to protected `main`, path `/docs`. Website changes follow the same PR process as source.

## Videos

Yes: add an authentic short screen recording demonstrating schematic editing, PCB work, the 3D viewer and/or an assistant's preview/commit workflow. No demonstration video has been recorded or uploaded as part of this link setup.

- A YouTube/Vimeo video can be linked from a labelled thumbnail in the README. It opens the video service; a GitHub README does not run arbitrary third-party iframe embeds.
- GitHub supports video attachments such as MP4, MOV and WebM. Upload through the supported Markdown attachment interface and retain the generated URL; verify playback in the rendered README. Platform size/codec limits apply.
- A downloadable demo MP4 can also be a release asset. Keep large recordings out of ordinary Git history. Public release assets require a published release; draft videos remain private to authorised users.
- Use original/sanitized demo projects. Remove tokens, personal paths and customer designs. Identify version, human/AI actions and approximate/generated visuals accurately; do not present a conceptual animation as proof that the application performed a task.
- Provide captions/transcript and a brief written equivalent so the demo remains useful without audio.

Attachment reference: https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/attaching-files
