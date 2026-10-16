# Third-party notices

## Publication status and audit boundary

The public 0.8.0 early-access release includes a [native-runtime notice/source supplement](https://github.com/JeremyKariukicodesnow/Jenga/releases/download/v0.8.0/Jenga-native-10.0.6-NOTICES-SOURCES.zip). It maps the 209 unmodified KiCad 10.0.6 native files to 40 components, retains upstream license material and provides exact source/access locations plus pinned build/dependency recipes. Microsoft system runtimes retain their separate redistribution terms. This is a technical provenance/licensing-material record, not an independent legal or security audit.

The [component catalog](docs/native-runtime-components.json) is readable in the repository; the supplement contains its referenced notice files and recipes. [The original machine-generated inventory](docs/native-runtime-inventory.json) remains a raw metadata capture: its original pending review fields are not the subsequent component/source record. Recreate that raw capture with `scripts/inventory-native-runtime.ps1`. `scripts/verify-native-notices.ps1` checks the actual runtime hashes and notice coverage before future packaging; it does not replace legal judgment. Future app/source packages include `backend/kicad/NOTICES-SOURCES.zip`; preserved original 0.7/0.8 installers keep their original bytes and receive the supplement beside them on the release page.

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
