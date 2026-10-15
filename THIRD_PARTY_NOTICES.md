# Third-party notices

## Publication status and audit boundary

This inventory identifies observed upstream material; it is not a claim that every native DLL's redistribution obligations have already been fulfilled. The backend preparation script currently copies 208 DLLs from the official local KiCad installation. Exact runtime versions, per-component license texts and matching corresponding-source/rebuild locations must be completed before public binary distribution. See [the public release gate](docs/PUBLIC_RELEASE_CHECKLIST.md). Keep Windows downloads in a draft until that gate is satisfied.

Observed source reuse in this repository includes **KiCad NewStroke font data**, not just compatible file formats. Original notices remain in the included source. The Rust parser, editor and font decoder are maintained as Jenga code; this statement does not prove that every historical line was independently authored. Future copied/adapted code must be identified explicitly, even if translated to another language.

Maintainer/project ownership and AI-development acknowledgements are in [AUTHORS.md](AUTHORS.md); they do not replace upstream copyright attribution or transfer ownership of third-party work.

The Jenga branding image in `resources/branding` was supplied by the project owner for this application. Original dot lettering and outlined toolbar icons are rendered by Jenga; no proprietary Nothing or Windows font is redistributed.

Jenga bundles official KiCad symbols, footprints and the complete 3D model library from the local KiCad 10.0 installation in release packages. Exact payload hashes and provenance are recorded in resources/libraries. Library license texts and the KiCad exception are included in libraries/licenses; library data is unmodified.

Jenga embeds the unmodified KiCad NewStroke font data from tag 10.0.0, common/newstroke_font.cpp. Copyright (C) 2010 Vladimir Uryvaev and the KiCad developers. The file is GPL-2.0-or-later with additional MIT and SIL Open Font License notices for CJK data; all notices remain in resources/fonts/newstroke_font.cpp, accompanied by GPL-2.0.txt and upstream AUTHORS.txt. The Rust font decoder is Jenga's implementation. Upstream: https://github.com/KiCad/kicad-source-mirror/blob/10.0.0/common/newstroke_font.cpp

Rust dependencies keep their upstream licenses. Release packages include a generated dependency inventory and bundled dependency source/licenses alongside Jenga source.

Before adding third-party material, record its origin, version or commit, applicable license, copyright notices, and modifications, and include required license texts.

## KiCad source

Most KiCad source is GPL-3.0-or-later. Individual files and dependencies may carry other terms; inspect the exact material before reuse. Preserve attribution and license notices, identify modifications, and satisfy corresponding-source requirements when distributing covered binaries. Translating covered C++ code into Rust does not remove its licensing obligations.

Source license information: https://www.kicad.org/about/licenses/

## KiCad libraries

Official KiCad libraries use CC-BY-SA-4.0 with the KiCad libraries exception. When bundling them, retain the license, exception, attribution, and applicable share-alike obligations for library adaptations. The exception permits use of library data in commercial, closed, and non-commercial designs; library redistribution remains subject to its terms.

Third-party or locally installed libraries may use different licenses. Discovering a local library does not grant permission to redistribute it.

Library license and exception: https://www.kicad.org/libraries/license/


Jenga 0.7 Windows packages include an unmodified KiCad 10.0.6 CLI and native runtime DLLs as a separate process backend. GPLv3-or-later and third-party terms are in `backend/kicad/COPYRIGHT.txt`; corresponding source and Windows dependency build-recipe locations are in `backend/kicad/SOURCE_AND_LICENSES.txt`. Official component 3D library assets are distributed under CC-BY-SA-4.0 with the KiCad libraries exception, with upstream attribution and hashes in libraries/manifest.json and libraries/SHA256SUMS.
